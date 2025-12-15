// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {BtcRelay} from "../../src/relay/BtcRelay.sol";

/**
 * @title SubmitBitcoinHeaders
 * @notice Submit Bitcoin block headers to BtcRelay
 * @dev Reads headers from environment and submits them
 */
contract SubmitBitcoinHeaders is Script {
    function run() external {
        uint256 pk = vm.envUint("PRIVATE_KEY");
        address relay = vm.envAddress("BTC_RELAY_ADDRESS");

        console2.log("");
        console2.log("========================================");
        console2.log("Submitting Bitcoin Headers to BtcRelay");
        console2.log("========================================");
        console2.log("");
        console2.log("BtcRelay:", relay);
        console2.log("Operator:", vm.addr(pk));
        console2.log("");

        BtcRelay btcRelay = BtcRelay(relay);

        // Check current state
        (bytes32 bestHash, uint256 bestHeight, uint256 bestChainWork) = btcRelay
            .getBestBlock();
        console2.log("Current best block:");
        console2.log("  Hash:", vm.toString(bestHash));
        console2.log("  Height:", bestHeight);
        console2.log("  Chain Work:", bestChainWork);
        console2.log("");

        vm.startBroadcast(pk);

        // Read headers from environment
        // Format: HEADER_1, HEADER_2, ..., HEADER_N
        // Each header is 80 bytes (160 hex characters)

        uint256 headerCount = vm.envUint("HEADER_COUNT");
        console2.log("Processing", headerCount, "headers...");
        console2.log("");

        uint256 submitted = 0;
        uint256 skipped = 0;

        for (uint256 i = 1; i <= headerCount; i++) {
            string memory envKey = string(
                abi.encodePacked("HEADER_", vm.toString(i))
            );
            bytes memory header = vm.envBytes(envKey);

            uint256 height = vm.envUint(
                string(abi.encodePacked("HEIGHT_", vm.toString(i)))
            );

            // Calculate block hash
            bytes32 blockHash = sha256(abi.encodePacked(sha256(header)));

            // Skip if height <= current best height (likely already exists)
            if (height <= bestHeight) {
                skipped++;
                continue;
            }

            console2.log("  Submitting block", i, "of", headerCount);
            console2.log("    Height:", height);
            console2.log("    Hash:", vm.toString(blockHash));

            btcRelay.submitBlockHeader(header, height);
            submitted++;

            console2.log("    [OK] Submitted");
        }

        console2.log("");
        console2.log("Summary:");
        console2.log("  Submitted:", submitted);
        console2.log("  Skipped:", skipped);

        vm.stopBroadcast();

        // Check updated state
        (
            bytes32 newBestHash,
            uint256 newBestHeight,
            uint256 newChainWork
        ) = btcRelay.getBestBlock();
        (bytes32 finalizedHash, uint256 finalizedHeight) = btcRelay
            .getFinalizedBlock();

        console2.log("");
        console2.log("========================================");
        console2.log("Submission Complete!");
        console2.log("========================================");
        console2.log("");
        console2.log("Best Block:");
        console2.log("  Hash:", vm.toString(newBestHash));
        console2.log("  Height:", newBestHeight);
        console2.log("  Chain Work:", newChainWork);
        console2.log("");
        console2.log("Finalized Block:");
        console2.log("  Hash:", vm.toString(finalizedHash));
        console2.log("  Height:", finalizedHeight);
        console2.log("");
    }
}
