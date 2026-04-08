// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";

/// @title HiveCallRegistry
/// @dev Maps Solana vault id (bytes32) to Arbitrum VaultExecutor address.
contract HiveCallRegistry is Ownable {
    mapping(bytes32 vaultId => address executor) public executorOf;

    event ExecutorRegistered(bytes32 indexed vaultId, address executor);

    constructor(address initialOwner) Ownable(initialOwner) {}

    function register(bytes32 vaultId, address executor) external onlyOwner {
        require(executor != address(0), "Zero executor");
        executorOf[vaultId] = executor;
        emit ExecutorRegistered(vaultId, executor);
    }
}
