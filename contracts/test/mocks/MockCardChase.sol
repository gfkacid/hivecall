// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {ICardChase} from "../../src/interfaces/ICardChase.sol";

/// @dev Minimal mock for Foundry tests.
contract MockCardChase is ICardChase {
    uint256 private _nextId = 1;

    function openPosition(bytes32, bool, uint256) external returns (uint256 positionId) {
        positionId = _nextId++;
    }

    function closePosition(uint256) external pure returns (uint256 payout) {
        return 0;
    }

    function getMarket(bytes32) external pure returns (bool, uint256, uint256) {
        return (true, type(uint256).max, type(uint256).max);
    }

    function getPosition(uint256) external pure returns (bool, uint256, bool, uint256) {
        return (true, 0, false, 0);
    }
}
