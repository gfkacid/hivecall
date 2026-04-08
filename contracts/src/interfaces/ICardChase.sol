// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @notice CardChase prediction market (Arbitrum) — see protocol spec.
interface ICardChase {
    function openPosition(bytes32 marketId, bool isLong, uint256 usdcAmount)
        external
        returns (uint256 positionId);

    function closePosition(uint256 positionId) external returns (uint256 payout);

    function getMarket(bytes32 marketId)
        external
        view
        returns (bool isOpen, uint256 openInterest, uint256 expiresAt);

    function getPosition(uint256 positionId)
        external
        view
        returns (bool isLong, uint256 size, bool isSettled, uint256 payout);
}
