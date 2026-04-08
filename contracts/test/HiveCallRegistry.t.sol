// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";

import {HiveCallRegistry} from "../src/HiveCallRegistry.sol";

contract HiveCallRegistryTest is Test {
    HiveCallRegistry internal registry;

    function setUp() public {
        registry = new HiveCallRegistry(address(this));
    }

    function test_register_sets_executor() public {
        bytes32 vid = keccak256("vault-1");
        address exec = address(0xBEEF);
        registry.register(vid, exec);
        assertEq(registry.executorOf(vid), exec);
    }
}
