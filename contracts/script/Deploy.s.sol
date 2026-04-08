// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console2} from "forge-std/Script.sol";

import {HiveCallRegistry} from "../src/HiveCallRegistry.sol";
import {VaultExecutorFactory} from "../src/VaultExecutorFactory.sol";

/// @notice Deploy registry + factory. Set `PRIVATE_KEY` and RPC in env; see README.
contract DeployScript is Script {
    function run() external {
        uint256 pk = vm.envUint("PRIVATE_KEY");
        vm.startBroadcast(pk);

        address deployer = vm.addr(pk);
        HiveCallRegistry registry = new HiveCallRegistry(deployer);
        VaultExecutorFactory factory = new VaultExecutorFactory(deployer);

        console2.log("HiveCallRegistry", address(registry));
        console2.log("VaultExecutorFactory", address(factory));

        vm.stopBroadcast();
    }
}
