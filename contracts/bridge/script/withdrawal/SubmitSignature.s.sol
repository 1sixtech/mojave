// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import "forge-std/Script.sol";
import "../../src/BridgeGateway.sol";

/**
 * @title SubmitSignature Script
 * @notice Submit a single operator signature for a withdrawal with automatic signature generation
 * @dev This script:
 *      1. Generates EIP-712 approval digest from withdrawal details
 *      2. Signs the digest using the operator's private key
 *      3. Submits signature to BridgeGateway
 *      4. Auto-finalizes if threshold reached and rawTx provided
 */
contract SubmitSignature is Script {
    function run() external {
        // Environment variables
        uint256 operatorKey = vm.envUint("OPERATOR_KEY");
        address bridgeAddress = vm.envAddress("BRIDGE_ADDRESS");
        bytes32 wid = vm.envBytes32("WID");

        // Optional: provide pre-computed signature (for testing)
        bytes memory providedSignature = "";
        try vm.envBytes("SIGNATURE") returns (bytes memory sig) {
            providedSignature = sig;
        } catch {}
        // rawTx: empty bytes for incremental signing, or full rawTx for final signature
        bytes memory rawTx = "";
        try vm.envBytes("RAW_TX") returns (bytes memory _rawTx) {
            rawTx = _rawTx;
        } catch {}
        BridgeGateway bridge = BridgeGateway(bridgeAddress);

        console.log("");
        console.log("========================================");
        console.log("Submitting Single Operator Signature");
        console.log("========================================");
        console.log("");
        console.log("Bridge:", bridgeAddress);
        console.log("WID:", vm.toString(wid));
        console.log("Operator:", vm.addr(operatorKey));

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
            "(1=Pending, 2=Ready, 3=Finalized)"
        );
        console.log("  SignerSetId:", signerSetId);
        console.log("  OutputsHash:", vm.toString(outputsHash));

        require(
            state == BridgeGateway.WState.Pending,
            "Withdrawal must be in Pending state"
        );

        // Generate or use provided signature
        bytes memory signature;
        if (providedSignature.length > 0) {
            console.log("");
            console.log(
                "Using provided signature (length:",
                providedSignature.length,
                ")"
            );
            signature = providedSignature;
        } else {
            console.log("");
            console.log("Generating EIP-712 signature...");

            // CRITICAL: Must use withdrawal deadline (not arbitrary expiry)
            // submitSignature verifies with _approvalDigest(..., w.deadline, ...)
            console.log("Using withdrawal deadline:", deadline);

            bytes32 approvalDigest = bridge.approvalDigestPublic(
                wid,
                outputsHash,
                version,
                deadline, // ← Use withdrawal deadline
                signerSetId
            );

            console.log("Approval digest:", vm.toString(approvalDigest));

            // Sign the digest
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(
                operatorKey,
                approvalDigest
            );
            signature = abi.encodePacked(r, s, v);

            console.log("Signature generated (length:", signature.length, ")");
        }

        console.log("");
        if (rawTx.length > 0) {
            console.log("Mode: FINAL signature with rawTx");
            console.log("  -> Will auto-finalize if threshold reached");
            console.log("  -> RawTx length:", rawTx.length);
        } else {
            console.log("Mode: INCREMENTAL signature (no rawTx)");
            console.log("  -> Will not finalize, just add signature");
        }

        // Submit signature
        console.log("");
        console.log("Submitting signature to BridgeGateway...");

        vm.startBroadcast(operatorKey);

        try bridge.submitSignature(wid, signature, rawTx) {
            console.log("[SUCCESS] Signature submitted!");
        } catch Error(string memory reason) {
            console.log("[FAILED]:", reason);
            vm.stopBroadcast();
            revert(reason);
        } catch (bytes memory lowLevelData) {
            console.log("[FAILED] Low-level error:");
            console.logBytes(lowLevelData);
            vm.stopBroadcast();
            revert("Signature submission failed");
        }
        vm.stopBroadcast();

        // Check updated withdrawal status
        console.log("");
        console.log("Checking updated withdrawal status...");

        try bridge.getWithdrawalDetails(wid) returns (
            address,
            uint256,
            bytes memory,
            uint64,
            bytes32,
            uint32,
            uint32,
            BridgeGateway.WState newState
        ) {
            console.log("New withdrawal status:", uint256(newState));

            if (newState == BridgeGateway.WState.Finalized) {
                console.log("");
                console.log("*************************************");
                console.log("* AUTO-FINALIZED! *");
                console.log("* Threshold reached, wBTC burned! *");
                console.log("* Ready for Bitcoin broadcast *");
                console.log("*************************************");
            } else if (newState == BridgeGateway.WState.Ready) {
                console.log("");
                console.log(
                    "[READY] Threshold reached, awaiting finalization call"
                );
            } else if (newState == BridgeGateway.WState.Pending) {
                console.log("");
                console.log("[PENDING] More signatures needed");
            }
        } catch {
            console.log("Could not fetch updated status");
        }
        console.log("");
        console.log("========================================");
        console.log("Signature Submission Complete");
        console.log("========================================");
        console.log("");
    }
}
