// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @notice Circle CCTP TokenMessenger (subset used by HiveCall).
interface ITokenMessenger {
    function depositForBurn(
        uint256 amount,
        uint32 destinationDomain,
        bytes32 mintRecipient,
        address burnToken
    ) external returns (uint64 nonce);
}
