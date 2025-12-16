// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import "forge-std/Script.sol";
import {BridgeGateway} from "../../src/BridgeGateway.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

interface WBTC is IERC20 {}

/**
 * @title RequestWithdrawalWithUtxoIds
 * @notice User requests withdrawal with UTXO IDs obtained from UtxoRegistered events
 * @dev Uses event-sourced UTXO IDs
 *
 * Usage:
 *   export RECIPIENT_KEY=0x...
 *   export BRIDGE_ADDRESS=0x...
 *   export WBTC_ADDRESS=0x...
 *   export RECIPIENT=0x...
 *   export WITHDRAW_AMOUNT=25000
 *   export WITHDRAW_DEST_SPK=0x...
 *   export UTXO_ID_0=0x...  # Primary UTXO ID from UtxoRegistered event
 *   export UTXO_AMOUNT_0=50000
 *   # Optional: Add more UTXOs
 *   # export UTXO_ID_1=0x...
 *   # export UTXO_AMOUNT_1=100000
 *
 *   forge script script/RequestWithdrawalWithUtxoIds.s.sol:RequestWithdrawalWithUtxoIds \
 *       --broadcast --rpc-url $MOJAVE_RPC_URL
 */
contract RequestWithdrawalWithUtxoIds is Script {
    function run() external {
        uint256 userKey = vm.envUint("RECIPIENT_KEY");
        address bridgeAddress = vm.envAddress("BRIDGE_ADDRESS");
        address wbtcAddress = vm.envAddress("WBTC_ADDRESS");
        address recipient = vm.envAddress("RECIPIENT");

        uint256 amountSats = vm.envUint("WITHDRAW_AMOUNT");
        bytes memory destScriptPubkey;
        try vm.envBytes("WITHDRAW_DEST_SPK") returns (bytes memory spk) {
            destScriptPubkey = spk;
        } catch {
            revert("WITHDRAW_DEST_SPK not set or invalid");
        }
        uint64 deadline = uint64(block.timestamp + 86400); // 24 hours

        console.log(
            "=== Requesting Withdrawal with UTXO IDs (Event-Sourced) ==="
        );
        console.log("Bridge:", bridgeAddress);
        console.log("WBTC:", wbtcAddress);
        console.log("User:", recipient);
        console.log("Withdrawal amount:", amountSats, "sats");
        console.log("Deadline:", deadline);
        console.log("");

        // Load UTXOs from environment (UTXO_ID_0, UTXO_ID_1, etc.)
        // Try to load up to 10 UTXOs
        uint256 utxoCount = 0;
        BridgeGateway.UtxoInput[]
            memory tempUtxos = new BridgeGateway.UtxoInput[](10);

        for (uint256 i = 0; i < 10; i++) {
            string memory utxoIdKey = string(
                abi.encodePacked("UTXO_ID_", vm.toString(i))
            );
            string memory utxoAmountKey = string(
                abi.encodePacked("UTXO_AMOUNT_", vm.toString(i))
            );

            try vm.envBytes32(utxoIdKey) returns (bytes32 utxoId) {
                uint256 amount = vm.envUint(utxoAmountKey);

                // We need to reverse-lookup the UTXO details from the contract
                // TODO: the API would provide (txid, vout, amount) alongside utxoId
                // For now, we use a helper mapping or require env vars

                // Try to load TXID and VOUT for this UTXO
                string memory txidKey = string(
                    abi.encodePacked("UTXO_TXID_", vm.toString(i))
                );
                string memory voutKey = string(
                    abi.encodePacked("UTXO_VOUT_", vm.toString(i))
                );

                bytes32 txid = vm.envOr(txidKey, bytes32(0));
                uint32 vout = uint32(vm.envOr(voutKey, uint256(0)));

                if (txid == bytes32(0)) {
                    console.log(
                        "[WARNING] UTXO ID",
                        i,
                        "found but no TXID provided"
                    );
                    console.log(
                        "  Skipping UTXO (need TXID for withdrawal construction)"
                    );
                    continue;
                }

                tempUtxos[utxoCount] = BridgeGateway.UtxoInput({
                    txid: txid,
                    vout: vout,
                    amount: amount
                });

                console.log("UTXO", i, ":");
                console.log("  ID:", vm.toString(utxoId));
                console.log("  TXID:", vm.toString(txid));
                console.log("  VOUT:", vout);
                console.log("  Amount:", amount, "sats");

                utxoCount++;
            } catch {
                // No more UTXOs
                break;
            }
        }

        require(utxoCount > 0, "No UTXOs provided");

        // Create properly sized array
        BridgeGateway.UtxoInput[]
            memory proposedUtxos = new BridgeGateway.UtxoInput[](utxoCount);
        for (uint256 i = 0; i < utxoCount; i++) {
            proposedUtxos[i] = tempUtxos[i];
        }

        console.log("\nTotal UTXOs proposed:", utxoCount);
        console.log("");

        WBTC wbtc = WBTC(wbtcAddress);
        BridgeGateway bridge = BridgeGateway(bridgeAddress);

        // Check balances before
        uint256 userBalanceBefore = wbtc.balanceOf(recipient);
        uint256 bridgeBalanceBefore = wbtc.balanceOf(bridgeAddress);

        console.log("=== Before Withdrawal Request ===");
        console.log("User wBTC balance:", userBalanceBefore);
        console.log("Bridge wBTC balance:", bridgeBalanceBefore);
        console.log("");

        // Verify all UTXOs are unspent
        console.log("Verifying UTXO states...");
        for (uint256 i = 0; i < utxoCount; i++) {
            bytes32 utxoId = keccak256(
                abi.encodePacked(proposedUtxos[i].txid, proposedUtxos[i].vout)
            );
            bool isSpent = bridge.utxoSpent(utxoId);

            console.log("UTXO", i, ":");
            console.log("  Calculated ID:", vm.toString(utxoId));
            console.log("  Is spent:", isSpent);

            if (isSpent) {
                console.log("[ERROR] UTXO", i, "is already spent!");
                revert("UTXO already spent");
            }
        }
        console.log("All UTXOs are unspent");
        console.log("");

        vm.startBroadcast(userKey);

        // Approve wBTC to bridge
        console.log("Approving", amountSats, "wBTC to bridge...");
        wbtc.approve(bridgeAddress, amountSats);

        // Request withdrawal with proposed UTXOs
        console.log("Requesting withdrawal...");
        console.log("  Amount:", amountSats, "sats");
        console.log("  Destination scriptPubKey:");
        console.logBytes(destScriptPubkey);
        console.log("  Proposed UTXOs:", utxoCount);

        bytes32 wid = bridge.requestWithdraw(
            amountSats,
            destScriptPubkey,
            deadline,
            proposedUtxos
        );

        vm.stopBroadcast();

        console.log("\n=== Withdrawal Requested ===");
        console.log("Withdrawal ID:", vm.toString(wid));

        // Check balances after
        uint256 userBalanceAfter = wbtc.balanceOf(recipient);
        uint256 bridgeBalanceAfter = wbtc.balanceOf(bridgeAddress);

        console.log("\n=== After Withdrawal Request ===");
        console.log("User wBTC balance:", userBalanceAfter);
        console.log("Bridge wBTC balance:", bridgeBalanceAfter);
        console.log("wBTC burned:", userBalanceBefore - userBalanceAfter);
        console.log("");

        // Verify withdrawal was registered
        console.log("=== Verifying Withdrawal State ===");
        (
            address withdrawUser,
            uint256 withdrawAmount,
            , // destSpk
            uint64 withdrawDeadline,
            , // outputsHash
            , // version
            , // signerSetId
            , // state
            , // signatureBitmap
            , // signatureCount
            // Note: selectedUtxoIds[] array is not returned by the public getter
            uint256 totalInputAmount
        ) = bridge.withdrawals(wid);

        console.log("Withdrawal user:", withdrawUser);
        console.log("Withdrawal amount:", withdrawAmount);
        console.log("Withdrawal deadline:", withdrawDeadline);
        console.log("Total input amount:", totalInputAmount);
        console.log("");

        console.log("SUCCESS!");
        console.log("Withdrawal ID:", vm.toString(wid));
    }
}
