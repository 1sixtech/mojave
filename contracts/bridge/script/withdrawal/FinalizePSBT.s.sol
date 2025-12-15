// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

import "forge-std/Script.sol";
import "../../src/BridgeGateway.sol";

/**
 * @title FinalizePSBT
 * @notice Finalizes a withdrawal by submitting the fully signed Bitcoin transaction
 * @dev This script:
 *      1. Takes a WID (withdrawal ID)
 *      2. Takes a fully signed Bitcoin rawTx
 *      3. Calls submitSignature(wid, "", rawTx) from operator
 *      4. Contract validates rawTx outputs match PSBT outputsHash
 *      5. Contract finalizes withdrawal and emits SignedTxReady
 */
contract FinalizePSBT is Script {
    function run() external {
        // Environment variables
        address bridgeAddress = vm.envAddress("BRIDGE_ADDRESS");
        bytes32 wid = vm.envBytes32("WID");
        bytes memory rawTx = vm.envBytes("RAW_TX");
        uint256 operatorKey = vm.envUint("OPERATOR_KEY");

        BridgeGateway bridge = BridgeGateway(payable(bridgeAddress));

        console.log("");
        console.log("========================================");
        console.log("Finalizing Withdrawal with Signed Bitcoin TX");
        console.log("========================================");
        console.log("");
        console.log("Bridge:", bridgeAddress);
        console.log("WID:", vm.toString(wid));
        console.log("Operator:", vm.addr(operatorKey));
        console.log("RawTx length:", rawTx.length, "bytes");

        // Get withdrawal details
        (
            address user,
            uint256 amountSats,
            bytes memory destSpk,
            uint64 deadline,
            bytes32 outputsHash,
            uint32 version,
            uint32 signerSetId,
            BridgeGateway.WState state
        ) = bridge.getWithdrawalDetails(wid);

        console.log("");
        console.log("Withdrawal Details:");
        console.log("  User:", user);
        console.log("  Amount:", amountSats, "sats");
        console.log(
            "  State:",
            uint256(state),
            "(0=None,1=Pending,2=Ready,3=Finalized)"
        );
        console.log("  SignerSetId:", signerSetId);
        console.log("  OutputsHash:", vm.toString(outputsHash));

        require(
            state == BridgeGateway.WState.Ready,
            "Withdrawal must be in Ready state"
        );

        console.log("");
        console.log("Finalizing withdrawal...");
        console.log("  Submitting fully signed Bitcoin transaction");
        console.log("  Contract will verify outputs match PSBT");

        // Submit empty signature (already have threshold) with rawTx
        bytes memory emptySignature = "";

        vm.startBroadcast(operatorKey);

        try bridge.submitSignature(wid, emptySignature, rawTx) {
            console.log("");
            console.log("[SUCCESS] Withdrawal finalized!");
            console.log("  wBTC burned");
            console.log("  SignedTxReady event emitted");
            console.log("  Ready for Bitcoin broadcast");
        } catch Error(string memory reason) {
            console.log("");
            console.log("[FAILED]:", reason);
            vm.stopBroadcast();
            revert(reason);
        } catch (bytes memory lowLevelData) {
            console.log("");
            console.log("[FAILED] Low-level error:");
            console.logBytes(lowLevelData);
            vm.stopBroadcast();
            revert("Finalization failed");
        }
        vm.stopBroadcast();

        // Check final state
        console.log("");
        console.log("Checking final state...");

        try bridge.getWithdrawalDetails(wid) returns (
            address,
            uint256,
            bytes memory,
            uint64,
            bytes32,
            uint32,
            uint32,
            BridgeGateway.WState finalState
        ) {
            console.log("Final status:", uint256(finalState));

            if (finalState == BridgeGateway.WState.Finalized) {
                console.log("");
                console.log("*************************************");
                console.log("* WITHDRAWAL FINALIZED! *");
                console.log("* Ready for Bitcoin broadcast *");
                console.log("*************************************");
            }
        } catch {
            console.log("Could not fetch final status");
        }
        console.log("");
        console.log("========================================");
        console.log("Finalization Complete");
        console.log("========================================");
        console.log("");
    }
}
