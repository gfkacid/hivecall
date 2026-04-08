// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";

import {VaultExecutor} from "./VaultExecutor.sol";

/// @title VaultExecutorFactory
/// @dev Scaffold — deploys minimal VaultExecutor instances (upgrade to clones + registry in spec).
contract VaultExecutorFactory is Ownable {
    event ExecutorDeployed(bytes32 indexed vaultId, address executor);

    constructor(address initialOwner) Ownable(initialOwner) {}

    function deployExecutor(
        bytes32 vaultId,
        address operatorKey,
        address protocolAdmin,
        bytes32 solanaRecipient,
        address usdc,
        address cardChase,
        address cctpTokenMessenger
    ) external onlyOwner returns (address executor) {
        VaultExecutor e = new VaultExecutor();
        e.initialize(
            vaultId,
            operatorKey,
            protocolAdmin,
            solanaRecipient,
            usdc,
            cardChase,
            cctpTokenMessenger,
            address(this)
        );
        executor = address(e);
        emit ExecutorDeployed(vaultId, executor);
    }
}
