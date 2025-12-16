// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";

import {WBTC} from "../src/token/WBTC.sol";
import {BtcRelay} from "../src/relay/BtcRelay.sol";
import {BridgeGateway} from "../src/BridgeGateway.sol";

/**
 * @title Deploy
 * @notice Deploy WBTC, BtcRelay (with genesis), and BridgeGateway
 * @dev This deploys the actual contracts with Bitcoin SPV verification
 */
contract Deploy is Script {
    // Bitcoin regtest genesis block
    // Genesis hash (big-endian from sha256(sha256(header))): 06226e46111a0b59caaf126043eb5bbf28c34f3a5e332a1fc7b2b73cf188910f
    // Genesis hash (little-endian from bitcoin-cli): 0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206
    // We use big-endian in BtcRelay for consistency with sha256 output
    bytes32 constant GENESIS_HASH =
        0x06226e46111a0b59caaf126043eb5bbf28c34f3a5e332a1fc7b2b73cf188910f;
    bytes32 constant GENESIS_MERKLE_ROOT =
        0x4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b;
    uint256 constant GENESIS_HEIGHT = 0;
    uint64 constant GENESIS_TIMESTAMP = 1296688602;
    uint256 constant GENESIS_CHAIN_WORK = 2; // Difficulty = 1 for regtest

    function run() external {
        uint256 pk = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(pk);

        console2.log("");
        console2.log("========================================");
        console2.log("Deploying Bridge Contracts");
        console2.log("========================================");
        console2.log("");
        console2.log("Deployer:", deployer);
        console2.log("");

        vm.startBroadcast(pk);

        // 1. Deploy WBTC
        console2.log("[1/4] Deploying WBTC...");
        WBTC wbtc = new WBTC(deployer); // deployer becomes admin
        console2.log("  [OK] WBTC deployed at:", address(wbtc));
        console2.log("");

        // 2. Deploy BtcRelay with genesis block
        console2.log("[2/4] Deploying BtcRelay...");
        console2.log("  Genesis Hash:", vm.toString(GENESIS_HASH));
        console2.log("  Genesis Height:", GENESIS_HEIGHT);

        BtcRelay relay = new BtcRelay(
            deployer, // admin
            GENESIS_HASH,
            GENESIS_MERKLE_ROOT,
            GENESIS_HEIGHT,
            GENESIS_TIMESTAMP,
            GENESIS_CHAIN_WORK
        );
        console2.log("  [OK] BtcRelay deployed at:", address(relay));
        console2.log("  [OK] Genesis block initialized");
        console2.log("");

        // 3. Deploy BridgeGateway
        console2.log("[3/4] Deploying BridgeGateway...");

        bytes
            memory vaultChangeSpk = hex"5120aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        bytes
            memory anchorSpk = hex"5120bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        bool anchorRequired = true;
        bytes memory vaultScriptPk = vm.envBytes("VAULT_SPK");
        bytes memory opretTag = hex"4d4f4a31"; // "MOJ1"

        BridgeGateway bridge = new BridgeGateway(
            address(wbtc),
            vaultChangeSpk,
            anchorSpk,
            anchorRequired,
            vaultScriptPk,
            opretTag,
            address(relay)
        );
        console2.log("  [OK] BridgeGateway deployed at:", address(bridge));
        console2.log("");

        // 4. Setup operators (5 operators, 4-of-5 threshold)
        console2.log("[4/4] Setting up operators...");

        address[] memory operators = new address[](5);
        operators[0] = vm.addr(0xA11CE);
        operators[1] = vm.addr(0xB11CE);
        operators[2] = vm.addr(0xC11CE);
        operators[3] = vm.addr(0xD11CE);
        operators[4] = vm.addr(0xE11CE);

        bridge.createOperatorSet(1, operators, 4, true);
        console2.log("  [OK] Created operator set (4-of-5)");
        console2.log("");

        // Grant MINTER_ROLE to BridgeGateway
        console2.log("Granting MINTER_ROLE to BridgeGateway...");
        bytes32 MINTER_ROLE = wbtc.MINTER_ROLE();
        wbtc.grantRole(MINTER_ROLE, address(bridge));
        console2.log("  [OK] MINTER_ROLE granted");
        console2.log("");

        vm.stopBroadcast();

        console2.log("========================================");
        console2.log("Deployment Complete!");
        console2.log("========================================");
        console2.log("");
        console2.log("Contract Addresses:");
        console2.log("  WBTC:", address(wbtc));
        console2.log("  BtcRelay:", address(relay));
        console2.log("  BridgeGateway:", address(bridge));
        console2.log("");
        console2.log("Update your .env file with:");
        console2.log("");
        console2.log("WBTC_ADDRESS=", address(wbtc));
        console2.log("BTC_RELAY_ADDRESS=", address(relay));
        console2.log("BRIDGE_ADDRESS=", address(bridge));
        console2.log("");
    }
}
