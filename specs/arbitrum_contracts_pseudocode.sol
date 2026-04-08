// ============================================================
// HIVECALL — ARBITRUM CONTRACTS (Solidity Pseudocode)
// ============================================================
// Contracts:
//   1. VaultExecutor       — per-vault prediction executor
//   2. VaultExecutorFactory — deploys VaultExecutors
//   3. HiveCallRegistry    — tracks deployed executors
// ============================================================
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/proxy/Clones.sol";
import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

// ============================================================
// INTERFACES
// ============================================================

/// @notice CardChase prediction market interface (Arbitrum)
interface ICardChase {
    function openPosition(
        bytes32 marketId,
        bool    isLong,        // true = UP, false = DOWN
        uint256 usdcAmount
    ) external returns (uint256 positionId);

    function closePosition(uint256 positionId) external returns (uint256 payout);

    function getMarket(bytes32 marketId) external view returns (
        bool   isOpen,
        uint256 openInterest,
        uint256 expiresAt
    );

    function getPosition(uint256 positionId) external view returns (
        bool   isLong,
        uint256 size,
        bool   isSettled,
        uint256 payout
    );
}

/// @notice Circle CCTP TokenMessenger interface
interface ITokenMessenger {
    function depositForBurn(
        uint256 amount,
        uint32  destinationDomain,
        bytes32 mintRecipient,
        address burnToken
    ) external returns (uint64 nonce);
}

/// @notice Circle CCTP MessageTransmitter interface
interface IMessageTransmitter {
    function receiveMessage(
        bytes calldata message,
        bytes calldata attestation
    ) external returns (bool success);
}


// ============================================================
// 1. VaultExecutor
// ============================================================
// Deployed once per HiveCall vault (via VaultExecutorFactory).
// Accepts USDC from CCTP mint, interfaces with CardChase,
// and bridges USDC back to Solana via CCTP on withdrawal.
//
// Access control: all state-changing functions require a valid
// ECDSA signature from the vault's registered operatorKey,
// which is the HiveCall relayer for that vault.
// ============================================================

contract VaultExecutor is ReentrancyGuard, Pausable {
    using SafeERC20 for IERC20;
    using ECDSA     for bytes32;

    // ----------------------------------------------------------
    // Constants
    // ----------------------------------------------------------

    uint32  public constant SOLANA_CCTP_DOMAIN = 5;         // Circle's domain ID for Solana
    uint256 public constant MAX_POSITION_BPS   = 1500;      // 15% max of vault assets per market
    uint256 public constant MAX_OPEN_INTEREST_BPS = 1000;   // 10% max of CardChase market OI

    // ----------------------------------------------------------
    // State
    // ----------------------------------------------------------

    address public immutable usdc;
    address public immutable cardChase;
    address public immutable cctpTokenMessenger;
    address public immutable factory;

    bytes32 public vaultId;             // mirrors Solana vault UUID
    address public operatorKey;         // relayer address for this vault (can sign instructions)
    address public protocolAdmin;       // HiveCall protocol multisig (emergency controls)
    bytes32 public solanaRecipient;     // Solana vault USDC account (bytes32 for CCTP)

    uint256 public usdcBalance;         // current USDC held (mirrors total deposited via CCTP)
    uint256 public epoch;

    uint256 public constant LARGE_TX_THRESHOLD = 50_000e6; // 50,000 USDC
    uint256 public constant TIME_LOCK_DELAY    = 24 hours;

    // positionId => PositionInfo
    mapping(uint256 => PositionInfo) public positions;
    uint256[] public openPositionIds;

    // Time-lock for large bridge transactions
    struct PendingBridge {
        uint256 amount;
        uint256 readyAt;
        bool    executed;
    }
    mapping(bytes32 => PendingBridge) public pendingBridges;

    uint256 private nonce; // signature replay protection

    // ----------------------------------------------------------
    // Structs
    // ----------------------------------------------------------

    struct PositionInfo {
        bytes32  marketId;
        bool     isLong;
        uint256  size;
        bool     isOpen;
        bool     isSettled;
        uint256  payout;
        uint256  openedAt;
    }

    // ----------------------------------------------------------
    // Events
    // ----------------------------------------------------------

    event PositionOpened(bytes32 indexed marketId, bool isLong, uint256 amount, uint256 positionId);
    event PositionClosed(uint256 indexed positionId, uint256 payout, int256 pnl);
    event BridgeToSolanaInitiated(uint256 amount, bytes32 solanaRecipient, uint64 cctpNonce);
    event BridgeToSolanaQueued(bytes32 bridgeId, uint256 amount, uint256 readyAt);
    event USDCReceived(uint256 amount, uint256 newBalance);
    event OperatorKeyUpdated(address newOperator);
    event EmergencyPause(address by);

    // ----------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------
    // Called by VaultExecutorFactory (via Clone proxy).
    // ----------------------------------------------------------

    function initialize(
        bytes32 _vaultId,
        address _operatorKey,
        address _protocolAdmin,
        bytes32 _solanaRecipient,
        address _usdc,
        address _cardChase,
        address _cctpTokenMessenger,
        address _factory
    ) external {
        require(vaultId == bytes32(0), "Already initialized");
        vaultId             = _vaultId;
        operatorKey         = _operatorKey;
        protocolAdmin       = _protocolAdmin;
        solanaRecipient     = _solanaRecipient;
        usdc                = _usdc;
        cardChase           = _cardChase;
        cctpTokenMessenger  = _cctpTokenMessenger;
        factory             = _factory;
    }

    // ----------------------------------------------------------
    // MODIFIERS
    // ----------------------------------------------------------

    modifier onlyOperator(
        bytes memory payload,
        bytes memory signature,
        uint256 sigNonce
    ) {
        // Verify nonce to prevent replay attacks
        require(sigNonce == nonce, "Invalid nonce");
        nonce++;

        // Recover signer from payload hash
        bytes32 msgHash = keccak256(abi.encodePacked(payload, sigNonce))
            .toEthSignedMessageHash();
        address signer = msgHash.recover(signature);
        require(signer == operatorKey, "Unauthorized: invalid operator signature");
        _;
    }

    modifier onlyProtocolAdmin() {
        require(msg.sender == protocolAdmin, "Unauthorized: not protocol admin");
        _;
    }

    // ----------------------------------------------------------
    // openPosition
    // ----------------------------------------------------------
    // Relayer calls this after quorum/admin approval on Solana.
    // Requires operator signature over the prediction parameters.
    // ----------------------------------------------------------

    function openPosition(
        bytes32 marketId,
        bool    isLong,
        uint256 usdcAmount,
        bytes   calldata signature,
        uint256 sigNonce
    )
        external
        nonReentrant
        whenNotPaused
        onlyOperator(
            abi.encodePacked(vaultId, marketId, isLong, usdcAmount),
            signature,
            sigNonce
        )
    {
        require(usdcAmount > 0, "Amount must be greater than zero");
        require(IERC20(usdc).balanceOf(address(this)) >= usdcAmount, "Insufficient USDC balance");

        // Risk cap: max 15% of vault total assets
        uint256 totalAssets = IERC20(usdc).balanceOf(address(this)); // simplified; add offchain Solana balance if needed
        require(usdcAmount <= (totalAssets * MAX_POSITION_BPS) / 10_000, "Exceeds position risk cap");

        // Open interest cap: max 10% of CardChase market OI
        (, uint256 openInterest, ) = ICardChase(cardChase).getMarket(marketId);
        require(
            usdcAmount <= (openInterest * MAX_OPEN_INTEREST_BPS) / 10_000,
            "Exceeds market open interest cap"
        );

        // Approve and open position on CardChase
        IERC20(usdc).safeApprove(cardChase, usdcAmount);
        uint256 positionId = ICardChase(cardChase).openPosition(marketId, isLong, usdcAmount);

        positions[positionId] = PositionInfo({
            marketId:  marketId,
            isLong:    isLong,
            size:      usdcAmount,
            isOpen:    true,
            isSettled: false,
            payout:    0,
            openedAt:  block.timestamp
        });
        openPositionIds.push(positionId);

        usdcBalance -= usdcAmount; // committed to CardChase

        emit PositionOpened(marketId, isLong, usdcAmount, positionId);
    }

    // ----------------------------------------------------------
    // closePosition
    // ----------------------------------------------------------
    // Called by relayer when CardChase market settles.
    // Winnings remain on Arbitrum in this contract.
    // ----------------------------------------------------------

    function closePosition(
        uint256 positionId,
        bytes   calldata signature,
        uint256 sigNonce
    )
        external
        nonReentrant
        whenNotPaused
        onlyOperator(
            abi.encodePacked(vaultId, positionId),
            signature,
            sigNonce
        )
    {
        PositionInfo storage pos = positions[positionId];
        require(pos.isOpen, "Position not open");

        uint256 payout = ICardChase(cardChase).closePosition(positionId);

        int256 pnl = int256(payout) - int256(pos.size);

        pos.isOpen    = false;
        pos.isSettled = true;
        pos.payout    = payout;

        // Payout stays in contract — updates balance
        usdcBalance += payout;

        // Remove from open positions array
        _removeFromOpenPositions(positionId);

        emit PositionClosed(positionId, payout, pnl);
    }

    // ----------------------------------------------------------
    // bridgeToSolana
    // ----------------------------------------------------------
    // Burns USDC via CCTP and sends to the vault's Solana
    // USDC account. Called by relayer when a withdrawal
    // on Solana exceeds the Solana-side balance.
    //
    // Large amounts (>50k USDC) are time-locked 24 hours.
    // ----------------------------------------------------------

    function bridgeToSolana(
        uint256 amount,
        bytes   calldata signature,
        uint256 sigNonce
    )
        external
        nonReentrant
        whenNotPaused
        onlyOperator(
            abi.encodePacked(vaultId, amount, "bridge"),
            signature,
            sigNonce
        )
    {
        require(amount > 0, "Zero amount");
        require(IERC20(usdc).balanceOf(address(this)) >= amount, "Insufficient balance");

        if (amount >= LARGE_TX_THRESHOLD) {
            // Time-lock: queue the bridge, execute after delay
            bytes32 bridgeId = keccak256(abi.encodePacked(vaultId, amount, block.timestamp));
            pendingBridges[bridgeId] = PendingBridge({
                amount:   amount,
                readyAt:  block.timestamp + TIME_LOCK_DELAY,
                executed: false
            });
            emit BridgeToSolanaQueued(bridgeId, amount, block.timestamp + TIME_LOCK_DELAY);
        } else {
            _executeBridgeToSolana(amount);
        }
    }

    // ----------------------------------------------------------
    // executePendingBridge
    // ----------------------------------------------------------
    // Executes a time-locked bridge after the delay has passed.
    // ----------------------------------------------------------

    function executePendingBridge(
        bytes32 bridgeId,
        bytes   calldata signature,
        uint256 sigNonce
    )
        external
        nonReentrant
        whenNotPaused
        onlyOperator(
            abi.encodePacked(bridgeId),
            signature,
            sigNonce
        )
    {
        PendingBridge storage pending = pendingBridges[bridgeId];
        require(!pending.executed, "Already executed");
        require(block.timestamp >= pending.readyAt, "Time lock not expired");

        pending.executed = true;
        _executeBridgeToSolana(pending.amount);
    }

    // ----------------------------------------------------------
    // receiveFromCCTP
    // ----------------------------------------------------------
    // Called automatically when CCTP mints USDC to this contract
    // (Solana → Arbitrum bridge). No access control needed —
    // only CCTP MessageTransmitter can mint USDC to this address.
    // Balance update tracked here for accounting.
    // ----------------------------------------------------------

    function notifyUSDCReceived(uint256 amount) external {
        // In practice, listen to USDC Transfer events from CCTP mint
        // or implement ERC20 receive hook.
        // This is a relayer-called accounting sync.
        require(msg.sender == factory || msg.sender == protocolAdmin, "Unauthorized");
        usdcBalance += amount;
        emit USDCReceived(amount, usdcBalance);
    }

    // ----------------------------------------------------------
    // updateOperatorKey
    // ----------------------------------------------------------
    // Allows vault admin (via protocolAdmin multisig) to rotate
    // the relayer/operator key for this vault.
    // ----------------------------------------------------------

    function updateOperatorKey(address newOperator) external onlyProtocolAdmin {
        require(newOperator != address(0), "Zero address");
        operatorKey = newOperator;
        emit OperatorKeyUpdated(newOperator);
    }

    // ----------------------------------------------------------
    // emergencyPause / unpause
    // ----------------------------------------------------------

    function emergencyPause() external onlyProtocolAdmin {
        _pause();
        emit EmergencyPause(msg.sender);
    }

    function unpause() external onlyProtocolAdmin {
        _unpause();
    }

    // ----------------------------------------------------------
    // getBalance (view)
    // ----------------------------------------------------------

    function getBalance() external view returns (uint256) {
        return IERC20(usdc).balanceOf(address(this));
    }

    function getOpenPositions() external view returns (uint256[] memory) {
        return openPositionIds;
    }

    function getPosition(uint256 positionId) external view returns (PositionInfo memory) {
        return positions[positionId];
    }

    // ----------------------------------------------------------
    // INTERNAL HELPERS
    // ----------------------------------------------------------

    function _executeBridgeToSolana(uint256 amount) internal {
        IERC20(usdc).safeApprove(cctpTokenMessenger, amount);
        uint64 cctpNonce = ITokenMessenger(cctpTokenMessenger).depositForBurn(
            amount,
            SOLANA_CCTP_DOMAIN,
            solanaRecipient,
            usdc
        );
        usdcBalance -= amount;
        emit BridgeToSolanaInitiated(amount, solanaRecipient, cctpNonce);
    }

    function _removeFromOpenPositions(uint256 positionId) internal {
        for (uint i = 0; i < openPositionIds.length; i++) {
            if (openPositionIds[i] == positionId) {
                openPositionIds[i] = openPositionIds[openPositionIds.length - 1];
                openPositionIds.pop();
                break;
            }
        }
    }
}


// ============================================================
// 2. VaultExecutorFactory
// ============================================================
// Deploys minimal Clone proxies of VaultExecutor for each
// new HiveCall vault. Called by the relayer when a
// CreateVaultEvent is detected on Solana.
// ============================================================

contract VaultExecutorFactory is Ownable {
    using Clones for address;

    address public immutable implementation;  // VaultExecutor implementation contract
    address public immutable usdc;
    address public immutable cardChase;
    address public immutable cctpTokenMessenger;
    address public immutable registry;

    event VaultExecutorDeployed(bytes32 indexed vaultId, address executor, address operatorKey);

    constructor(
        address _implementation,
        address _usdc,
        address _cardChase,
        address _cctpTokenMessenger,
        address _registry
    ) {
        implementation      = _implementation;
        usdc                = _usdc;
        cardChase           = _cardChase;
        cctpTokenMessenger  = _cctpTokenMessenger;
        registry            = _registry;
    }

    // ----------------------------------------------------------
    // deployVaultExecutor
    // ----------------------------------------------------------
    // Called by HiveCall relayer (owner) when a vault is created
    // on Solana. Deploys a Clone of VaultExecutor and registers
    // it in the HiveCallRegistry.
    //
    // operatorKey: the relayer's signing address for this vault
    // solanaRecipient: the vault's USDC token account on Solana
    //                  (as bytes32 for CCTP)
    // ----------------------------------------------------------

    function deployVaultExecutor(
        bytes32 vaultId,
        address operatorKey,
        address protocolAdmin,
        bytes32 solanaRecipient
    ) external onlyOwner returns (address executor) {
        // Deterministic address based on vaultId
        bytes32 salt = keccak256(abi.encodePacked(vaultId));
        executor = implementation.cloneDeterministic(salt);

        VaultExecutor(executor).initialize(
            vaultId,
            operatorKey,
            protocolAdmin,
            solanaRecipient,
            usdc,
            cardChase,
            cctpTokenMessenger,
            address(this)
        );

        IHiveCallRegistry(registry).registerVault(vaultId, executor);

        emit VaultExecutorDeployed(vaultId, executor, operatorKey);
    }

    // ----------------------------------------------------------
    // predictExecutorAddress (view)
    // ----------------------------------------------------------
    // Returns the deterministic address a VaultExecutor will
    // have before deployment. Useful for pre-funding.
    // ----------------------------------------------------------

    function predictExecutorAddress(bytes32 vaultId) external view returns (address) {
        bytes32 salt = keccak256(abi.encodePacked(vaultId));
        return implementation.predictDeterministicAddress(salt);
    }
}


// ============================================================
// 3. HiveCallRegistry
// ============================================================
// Central registry mapping Solana vault IDs to their Arbitrum
// VaultExecutor addresses. Used by the relayer and frontend
// to resolve vault addresses without off-chain lookups.
// ============================================================

contract HiveCallRegistry is Ownable {

    // vaultId => VaultExecutor address
    mapping(bytes32 => address) public vaultExecutors;

    // All registered vault IDs
    bytes32[] public vaultIds;

    address public factory;

    event VaultRegistered(bytes32 indexed vaultId, address executor);
    event FactorySet(address factory);

    modifier onlyFactory() {
        require(msg.sender == factory, "Only factory can register vaults");
        _;
    }

    function setFactory(address _factory) external onlyOwner {
        factory = _factory;
        emit FactorySet(_factory);
    }

    function registerVault(bytes32 vaultId, address executor) external onlyFactory {
        require(vaultExecutors[vaultId] == address(0), "Vault already registered");
        vaultExecutors[vaultId] = executor;
        vaultIds.push(vaultId);
        emit VaultRegistered(vaultId, executor);
    }

    function getExecutor(bytes32 vaultId) external view returns (address) {
        return vaultExecutors[vaultId];
    }

    function getAllVaults() external view returns (bytes32[] memory) {
        return vaultIds;
    }

    function isRegistered(bytes32 vaultId) external view returns (bool) {
        return vaultExecutors[vaultId] != address(0);
    }
}

interface IHiveCallRegistry {
    function registerVault(bytes32 vaultId, address executor) external;
}


// ============================================================
// NOTES ON INTEGRATION
// ============================================================
//
// CCTP Flow (Solana → Arbitrum):
//   1. Solana Vault Program calls CCTP TokenMessenger on Solana
//      (burn USDC)
//   2. Circle Attestation Service signs the burn proof
//   3. Relayer calls CCTP MessageTransmitter on Arbitrum
//      (receiveMessage) — mints USDC to VaultExecutor address
//   4. Relayer calls notifyUSDCReceived() to update accounting
//
// CCTP Flow (Arbitrum → Solana):
//   1. Relayer calls bridgeToSolana() on VaultExecutor
//   2. VaultExecutor calls CCTP TokenMessenger on Arbitrum
//      (depositForBurn, destination = Solana domain)
//   3. Circle Attestation Service signs burn proof
//   4. Relayer submits attestation to CCTP on Solana
//   5. USDC minted to vault's Solana USDC token account
//   6. Relayer calls finalize_withdrawal on Solana program
//
// Signature Scheme:
//   - operatorKey is an EVM address (secp256k1)
//   - All payloads include vaultId + nonce to prevent replay
//   - Relayer holds the operator private key per vault
//   - For V2+: consider a multisig operator key (Safe/Gnosis)
//
// Nonce Management:
//   - Each VaultExecutor maintains its own sequential nonce
//   - Relayer tracks the current nonce per vault
//   - Failed txs don't consume nonce — retry safe
//
// Risk Caps (enforced on-chain):
//   - Max 15% of vault balance per position
//   - Max 10% of CardChase market open interest per position
//   - Max $50k in a single bridge without 24hr time-lock
//
// Emergency Controls:
//   - protocolAdmin (multisig) can pause any VaultExecutor
//   - protocolAdmin can rotate operatorKey if relayer is compromised
//   - No admin can take user funds — only pause new operations
// ============================================================