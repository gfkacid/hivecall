// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @notice Circle CCTP MessageTransmitter (subset).
interface IMessageTransmitter {
    function receiveMessage(bytes calldata message, bytes calldata attestation)
        external
        returns (bool success);
}
