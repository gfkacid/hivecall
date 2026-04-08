HiveCall protocol spec

Overview
HiveCall is a Vault-Based Prediction Pooling protocol, specific to the TCG crowd.
Rather than individuals placing separate predictions, users deposit tokens into a vault that aggregates capital and places collective bets on CardChase markets(*). The vault's prediction logic can be governed by on-chain voting or weighted by token holdings — making it a "wisdom of the crowd" mechanism.
*CardChase is a prediction market for TCG prices on Arbitrum.

🏗️ Core Architecture Overview
The protocol has two distinct layers:

Vault Factory Layer — permissionless vault creation on Solana, each mirrored by a deployed contract on Arbitrum
Native Vault Layer — protocol-owned vault with token-staking-weighted voting and share mechanics

CCTP handles all USDC movement. A backend relayer handles off-chain vote aggregation and cross-chain instruction passing. All user-facing transactions on Solana are paid by users; relayed prediction transactions are paid by the protocol's admin wallet.

🏦 Vault System — Full Specification
Vault Creation Parameters
When anyone deploys a vault, they configure:
ParameterOptions / RangeNotesVisibilityPublic / PrivatePrivate requires a whitelistManagement TypeManaged / CrowdsourcedDetermines who picks markets and sizesDeposit CapAny USDC amount or unlimitedEnforced at deposit timePerformance Fee0–10%Taken from profits at epoch settlementFee AddressAny walletWhere performance fees are sentWhitelisted AddressesList of Solana walletsOnly applicable if vault is PrivateMinimum Quorum% of total vault sharesMinimum participation needed to place a bet
Each vault creation transaction deploys a corresponding smart contract on Arbitrum (via the relayer, gas paid by protocol admin wallet). The Arbitrum contract is access-controlled to only accept instructions signed by that vault's operator key.

Vault Types — Behavior Differences
Managed Vault
The vault admin unilaterally decides:

Which CardChase markets to enter
Position size per market
Direction (UP/DOWN)

The admin signs a message off-chain which the relayer picks up and executes. USDC is bridged via CCTP as needed. Shareholders deposit and earn based on outcomes but have no voting input.
Crowdsourced Vault
Voting power is proportional to USDC deposited. Shareholders vote off-chain (signed messages) on:

Which open CardChase markets to bet on
Direction of each prediction (UP or DOWN)

Each user gets their full voting power on every market — votes are independent per market, not split across them. The backend aggregates votes per market. If a market's vote weight meets the quorum threshold and has a clear directional majority, the prediction is placed. If quorum isn't met, no prediction is placed for that market that epoch.
Position sizing in crowdsourced vaults: proportional to the share of vault TVL that voted for that market (i.e. markets with higher participation get larger allocations, bounded by the risk cap).

Capital Flow — Deposit & Withdrawal
Deposits (Solana-side)

User deposits USDC into the Vault Program on Solana
They receive vault share tokens representing their proportional ownership
Gas paid by user in SOL
Deposits respect the vault's cap; private vaults check whitelist

Prediction Execution (Cross-chain)

When a prediction is triggered, the relayer checks the vault's current USDC balance on Arbitrum
If the required amount exceeds the Arbitrum balance, the delta is bridged via CCTP (Solana burn → Arbitrum mint)
The Arbitrum contract then calls CardChase with the bridged USDC

Winnings (Arbitrum-side)

Settled winnings remain on Arbitrum — no automatic bridging back
The Arbitrum contract tracks each vault's balance on Arbitrum
Future predictions draw from this balance first before bridging more from Solana

Withdrawals (Lazy Bridging)

User requests withdrawal on Solana
Protocol first checks: does the vault have enough USDC on Solana to cover the request?
If yes → pay directly from Solana balance
If no → bridge the delta from Arbitrum to Solana via CCTP (Arbitrum burn → Solana mint), then pay
Gas for the user's withdrawal transaction is paid by the user on Solana
Gas for the bridging operation (if needed) is paid by the protocol admin wallet

This lazy bridging approach minimizes unnecessary cross-chain round trips and keeps idle capital working on Arbitrum.

Gas Responsibility Summary
ActionWho Pays GasDeposit into vaultUser (Solana)Withdraw from vaultUser (Solana)Vote on marketFree (off-chain signed message)Place prediction (relayed)Protocol admin wallet (Solana + Arbitrum)CCTP bridge for predictionsProtocol admin walletCCTP bridge for withdrawalsProtocol admin walletVault creationUser (Solana) + Protocol admin (Arbitrum deploy)

Off-Chain Voting & Relayer Backend
The backend is responsible for:

Vote Collection — users submit signed messages (not transactions) specifying market + direction. Signatures are verified against their Solana wallet.
Vote Aggregation — per vault, per market, the backend tallies weighted votes (weight = USDC deposited). For the native vault, weight calculation is different (see below).
Quorum Check — if total vote weight on a market ≥ vault's quorum threshold, and one direction has majority, the prediction is triggered.
Relaying — the backend's admin wallet sends the trigger transaction on Solana (to update state) and the instruction to the Arbitrum contract to execute on CardChase.
Settlement Monitoring — the backend monitors CardChase for market resolutions and updates vault balances accordingly.

The backend is the V1 trust assumption — it should be open-sourced and have its vote aggregation logic publicly verifiable even if it's centralized. Decentralizing this is a V3 milestone.

🌟 Native Protocol Vault — Full Specification
The native vault is special in three ways: voting power blends USDC deposited with native token staked, share ownership follows the same blended logic, and there's a minimum deposit of 10 USDC to participate in governance.
Voting Power Formula
User Voting Power = USDC_deposited × (1 + Staking_Multiplier)

Staking_Multiplier = min(tokens_staked / multiplier_cap, max_multiplier)
Example configuration:

multiplier_cap = 10,000 tokens staked for max boost
max_multiplier = 2.0 (i.e. staking maxes out at a 3× total multiplier)
A user with 1,000 USDC deposited and 5,000 tokens staked → multiplier = 1.0 → voting power = 2,000
A user with 1,000 USDC and 0 tokens staked → voting power = 1,000

Minimum 10 USDC deposit required to cast any vote. Below this threshold, deposits are accepted but carry no voting rights.
Share Ownership Formula
To tie economic ownership to staking as well, use the same blended weight for share calculation:
User Share % = User_Blended_Weight / Σ(All_Users_Blended_Weight)

Blended_Weight = USDC_deposited × (1 + Staking_Multiplier)
This means a staker with the same USDC deposit as a non-staker owns a larger share of the vault's winnings. This is the primary economic incentive to stake — not just better voting power, but a larger slice of profits.
Important nuance: when a user unstakes tokens mid-epoch, their share is recalculated at the next epoch boundary, not immediately. This prevents gaming by staking before settlement and unstaking after.
Native Vault Fee Drip (Tokenomics Integration)
The 1% creator fee from the native token (launched on bags.fm) continuously drips back into the native vault as USDC (or is swapped to USDC via a DEX route). This:

Grows the vault's bankroll organically with every token transaction
Means stakers and USDC depositors benefit directly from token trading volume
Creates a flywheel: more token activity → larger vault bankroll → better prediction capacity → more attractive returns → more depositors → more demand for the token

The drip happens on a rolling basis (e.g. daily), not per-transaction, to batch gas costs efficiently.

🪙 Native Token Staking Program
Staking Program — Core Features
Stake & Unstake

Users stake native tokens into the Staking Program (Solana program)
Unstaking has a 7-day unbonding period — tokens are locked and multipliers are lost immediately upon unbonding initiation
This prevents epoch-boundary gaming

Multiplier Tiers (for UX clarity)
Tokens StakedMultiplierEffective Voting/Share Boost00×Baseline (USDC only)1,0000.2×+20%2,5000.5×+50%5,0001.0×+100%10,0002.0×+200% (cap)
Staking Rewards
Stakers in the native vault earn their enhanced share of vault profits. They don't receive separate staking emissions — their reward is the boosted share of prediction winnings + the growing bankroll from the fee drip. This keeps the tokenomics clean: the token has value because it boosts your vault returns, not because of inflationary emissions.
Staking Program Accounts (Solana)
Each user's stake is tracked in a StakeAccount PDA holding:

staker pubkey
amount_staked
stake_timestamp
unbonding_amount + unbonding_since (if unbonding)
current_multiplier (derived, cached for efficiency)


🖥️ Frontend Specification
Pages & Features
Homepage / Vault Discovery

Grid of all public vaults with: TVL, current epoch performance, vault type (managed/crowdsourced), performance fee, number of shareholders
Filter by: game (MTG/PKM/OP), type, TVL, performance
Featured section for the native protocol vault

Account Registration

Connect Solana wallet → create a user profile (stored off-chain, signed by wallet)
Profile tracks: voting history, prediction accuracy (for future reputation system), vaults deposited in

Vault Creation Flow

Step-by-step wizard:

Name & description
Visibility (Public / Private) → if Private, input whitelist addresses
Management type (Managed / Crowdsourced)
Deposit cap, performance fee (slider 0–10%), fee address
Quorum threshold (only if Crowdsourced)
Review & deploy → user pays Solana transaction fee



Vault Detail Page
For each vault the user can see:

Current TVL, their deposit, their share %, current epoch P&L
Open CardChase markets available to vote on (Crowdsourced) or current positions (Managed)
Voting panel: for each market, UP or DOWN button — one vote per market using full voting power
Deposit / Withdraw inputs
Epoch history with past performance

Native Vault Page
Same as vault detail but additionally shows:

User's staked token amount and current multiplier
Their effective voting power vs. if they had 0 tokens staked
Stake / Unstake panel with unbonding timer if active

Voting UX Detail
For crowdsourced vaults, each open CardChase market is shown as a card:

Card name / set
Current CardChase odds or price range
Aggregate vote tally (% UP vs % DOWN from current vault shareholders) — shown live
Current quorum progress bar (X% of shares have voted on this market)
UP / DOWN buttons — clicking submits a signed off-chain message, no gas


🗂️ Smart Contract / Program Interfaces
Solana Programs
Vault Factory Program
create_vault(config: VaultConfig) → VaultAccount
deposit(vault: Pubkey, amount: u64) → ShareTokens
withdraw(vault: Pubkey, shares: u64) → USDC
update_vault_config(vault: Pubkey, config: VaultConfig) // admin only
add_to_whitelist(vault: Pubkey, address: Pubkey) // admin only
Staking Program
stake(amount: u64) → StakeAccount
initiate_unstake(amount: u64) // starts 7-day unbonding
complete_unstake() // callable after unbonding period
get_multiplier(staker: Pubkey) → f64
Native Vault Program (extends Vault Factory)
deposit_native_vault(amount: u64) → BlendedShares
get_voting_power(user: Pubkey) → u64 // USDC × (1 + multiplier)
get_share_pct(user: Pubkey) → f64
receive_fee_drip(amount: u64) // called by fee drip mechanism
Arbitrum Contracts
Vault Executor (one deployed per vault)
solidityfunction executePrediction(market, direction, amount, operatorSig) external
function claimWinnings(marketId) external
function bridgeBack(amount) external // triggers CCTP burn
function getBalance() external view returns (uint256)
Access controlled: all state-changing functions require a valid signature from the vault's registered operator key.

🗺️ Revised Roadmap
PhaseDeliverablesV1 — CoreToken launch on bags.fm, native vault (manual admin for now), staking program, CCTP integration, basic frontendV2 — Vault FactoryPermissionless vault creation, managed + crowdsourced types, off-chain voting backend, relayer liveV3 — Full UXComplete frontend (vault discovery, voting UI, account profiles), lazy bridging, insurance fundV4 — DecentralizationRelayer decentralization, on-chain reputation scores, DAO governance for treasury