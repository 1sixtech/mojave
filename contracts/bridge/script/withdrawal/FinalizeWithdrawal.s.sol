// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "forge-std/Script.sol";
import "../../src/BridgeGateway.sol";
import {WBTC} from "../../src/token/WBTC.sol";

/**
 * @title FinalizeWithdrawal
 * @notice Finalize a withdrawal with operator signatures
 */
contract FinalizeWithdrawal is Script {
    function run() external {
        // Load WID from environment variable
        bytes32 wid = vm.envBytes32("WID");

        // Load environment variables
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address bridgeAddress = vm.envAddress("BRIDGE_ADDRESS");
        address wbtcAddress = vm.envAddress("WBTC_ADDRESS");
        address recipient = vm.envAddress("RECIPIENT");

        console.log("=== Finalizing Withdrawal ===");
        console.log("Bridge:", bridgeAddress);
        console.log("WID:");
        console.logBytes32(wid);

        WBTC wbtc = WBTC(wbtcAddress);
        BridgeGateway bridge = BridgeGateway(bridgeAddress);

        // Check balances before
        uint256 userBalanceBefore = wbtc.balanceOf(recipient);
        uint256 bridgeBalanceBefore = wbtc.balanceOf(bridgeAddress);
        uint256 supplyBefore = wbtc.totalSupply();

        console.log("\n=== Before Finalization ===");
        console.log("User wBTC balance:", userBalanceBefore);
        console.log("Bridge wBTC balance (locked):", bridgeBalanceBefore);
        console.log("Total wBTC supply:", supplyBefore);

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

        console.log("\n=== Withdrawal Details ===");
        console.log("User:", user);
        console.log("Amount:", amountSats, "sats");
        console.log("Deadline:", deadline);
        console.log("SignerSetId:", signerSetId);
        console.log("State:", uint8(state));
        console.log("OutputsHash:");
        console.logBytes32(outputsHash);

        // Build mock Bitcoin L1 transaction
        // Use actual withdrawal amount, with mock change/anchor amounts
        // TODO: these would be calculated based on vault UTXO selection
        bytes memory rawTx = buildMockBitcoinTx(
            bridge,
            amountSats, // exact amount to user
            140, // mock change amount
            1, // mock anchor amount
            destSpk
        );

        console.log("\nMock Bitcoin L1 TX:");
        console.logBytes(rawTx);

        // OutputsHash is already stored in the withdrawal
        console.log("\nUsing OutputsHash from withdrawal:");
        console.logBytes32(outputsHash);

        // Generate operator signatures
        // We need threshold signatures (e.g., 2-of-3 or 4-of-5)
        // version is already fetched from withdrawal details
        uint64 expiry = uint64(block.timestamp + 3600); // 1 hour

        bytes32 approvalDigest = bridge.approvalDigestPublic(
            wid,
            outputsHash,
            version,
            expiry,
            signerSetId
        );

        console.log("\nApproval digest:");
        console.logBytes32(approvalDigest);

        // Operator private keys (matching deployment)
        uint256[] memory operatorKeys = new uint256[](5);
        operatorKeys[0] = 0xA11CE;
        operatorKeys[1] = 0xB11CE;
        operatorKeys[2] = 0xC11CE;
        operatorKeys[3] = 0xD11CE;
        operatorKeys[4] = 0xE11CE;

        // Sign with first 4 operators (4-of-5 threshold)
        bytes[] memory sigs = new bytes[](4);
        uint256 signerBitmap = 0; // Will set bits 0,1,2,3

        for (uint256 i = 0; i < 4; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(
                operatorKeys[i],
                approvalDigest
            );
            sigs[i] = abi.encodePacked(r, s, v);
            signerBitmap |= (1 << i);

            console.log("Operator", i, "signed");
        }

        console.log("\nSigner bitmap:", signerBitmap);
        console.log("Total signatures:", sigs.length);

        // Finalize withdrawal
        vm.startBroadcast(deployerKey);

        console.log("\n=== Calling finalizeByApprovals ===");
        try
            bridge.finalizeByApprovals(
                wid,
                rawTx,
                outputsHash,
                version,
                signerSetId,
                signerBitmap,
                sigs,
                expiry
            )
        {
            console.log("[SUCCESS] Withdrawal finalized!");
        } catch Error(string memory reason) {
            console.log("[FAILED]:");
            console.log(reason);
        } catch (bytes memory lowLevelData) {
            console.log("[FAILED] with low-level error:");
            console.logBytes(lowLevelData);
        }
        vm.stopBroadcast();

        // Check balances after
        uint256 userBalanceAfter = wbtc.balanceOf(recipient);
        uint256 bridgeBalanceAfter = wbtc.balanceOf(bridgeAddress);
        uint256 supplyAfter = wbtc.totalSupply();

        console.log("\n=== After Finalization ===");
        console.log("User wBTC balance:", userBalanceAfter, "(unchanged)");
        console.log("Bridge wBTC balance:", bridgeBalanceAfter);
        console.log(
            "  Decreased by (BURNED):",
            bridgeBalanceBefore - bridgeBalanceAfter
        );
        console.log("Total wBTC supply:", supplyAfter);
        console.log("  Decreased by (BURNED):", supplyBefore - supplyAfter);

        console.log("\n[SUCCESS] Finalization complete!");
        console.log("Bitcoin L1 TX ready to broadcast");
    }

    function buildMockBitcoinTx(
        BridgeGateway bridge,
        uint256 userAmount,
        uint256 changeAmount,
        uint256 anchorAmount,
        bytes memory destSpk
    ) internal view returns (bytes memory) {
        // Simplified Bitcoin transaction structure
        // TODO: use proper Bitcoin transaction builder

        bytes memory btcTx = abi.encodePacked(
            hex"02000000", // version
            hex"01", // input count
            bytes32(0), // prev txid (mock)
            hex"00000000", // prev vout
            hex"00", // script sig length
            hex"ffffffff", // sequence
            hex"03" // output count (3 outputs)
        );

        // Get vault change and anchor SPKs from BridgeGateway
        bytes memory vaultChangeSpk = bridge.vaultChangeSpk();
        bytes memory anchorSpk = bridge.anchorSpk();

        // Output 0: User withdrawal (destSpk with userAmount)
        btcTx = abi.encodePacked(
            btcTx,
            _le64(uint64(userAmount)),
            _varint(destSpk.length),
            destSpk
        );

        // Output 1: Change to vault (vaultChangeSpk)
        btcTx = abi.encodePacked(
            btcTx,
            _le64(uint64(changeAmount)),
            _varint(vaultChangeSpk.length),
            vaultChangeSpk
        );

        // Output 2: Anchor (anchorSpk)
        btcTx = abi.encodePacked(
            btcTx,
            _le64(uint64(anchorAmount)),
            _varint(anchorSpk.length),
            anchorSpk,
            hex"00000000" // locktime
        );

        return btcTx;
    }

    // Bitcoin encoding helpers
    function _le16(uint16 x) internal pure returns (bytes memory) {
        return abi.encodePacked(uint8(x), uint8(x >> 8));
    }

    function _le32(uint32 x) internal pure returns (bytes memory) {
        return
            abi.encodePacked(
                uint8(x),
                uint8(x >> 8),
                uint8(x >> 16),
                uint8(x >> 24)
            );
    }

    function _le64(uint64 x) internal pure returns (bytes memory) {
        return
            abi.encodePacked(
                uint8(x),
                uint8(x >> 8),
                uint8(x >> 16),
                uint8(x >> 24),
                uint8(x >> 32),
                uint8(x >> 40),
                uint8(x >> 48),
                uint8(x >> 56)
            );
    }

    function _varint(uint x) internal pure returns (bytes memory) {
        if (x < 0xfd) return bytes.concat(bytes1(uint8(x)));
        if (x <= 0xffff) return bytes.concat(bytes1(0xfd), _le16(uint16(x)));
        if (x <= 0xffffffff)
            return bytes.concat(bytes1(0xfe), _le32(uint32(x)));
        return bytes.concat(bytes1(0xff), _le64(uint64(x)));
    }
}
