// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import "forge-std/Script.sol";

/**
 * @title CalculateDepositEnvelope (Real Bitcoin Integration)
 * @notice User calculates envelope hash for Bitcoin L1 deposit
 * @dev This is the ONLY calculation done by user's frontend
 *      User then uses their Bitcoin wallet to create the actual transaction
 */
contract CalculateDepositEnvelope is Script {
    function run() external view {
        // User's Mojave L2 address (where they want to receive wBTC)
        address recipient = vm.envAddress("RECIPIENT");
        uint256 depositAmount = vm.envUint("DEPOSIT_AMOUNT"); // Read from .env

        // Bridge contract info
        address bridgeAddress = vm.envAddress("BRIDGE_ADDRESS");
        uint256 chainId = block.chainid;

        // Build envelope (what goes in OP_RETURN)
        bytes4 opretTag = bytes4(vm.envBytes("OPRET_TAG"));

        // Use abi.encode for uint256 to get 32-byte big-endian, then extract and concatenate
        // Format: tag(4) + chainId(32) + bridge(20) + recipient(20) + amount(32) = 108 bytes
        bytes memory chainIdEncoded = abi.encode(chainId); // Always 32 bytes
        bytes memory amountEncoded = abi.encode(depositAmount); // Always 32 bytes

        bytes memory envelope = abi.encodePacked(
            opretTag, // 4 bytes
            chainIdEncoded, // 32 bytes (abi.encode ensures full 32 bytes)
            bridgeAddress, // 20 bytes
            recipient, // 20 bytes
            amountEncoded // 32 bytes (abi.encode ensures full 32 bytes)
        );
        // Total: 108 bytes

        bytes32 envelopeHash = keccak256(envelope);

        console.log("========================================");
        console.log("STEP 1: User Calculates Envelope");
        console.log("========================================");
        console.log("");

        console.log("[User Information]");
        console.log("Mojave L2 Recipient:", recipient);
        console.log("Deposit Amount:", depositAmount, "sats");
        console.log("");

        console.log("[Bridge Information]");
        console.log("Bridge Address:", bridgeAddress);
        console.log("Chain ID:", chainId);
        console.log("OP_RETURN Tag:", vm.toString(opretTag));
        console.log("");

        console.log("[Envelope for OP_RETURN]");
        console.log("Envelope (hex):", vm.toString(envelope));
        console.log("");

        console.log("[Envelope Hash]");
        console.log("Hash (for verification):", vm.toString(envelopeHash));
        console.log("");

        console.log("========================================");
        console.log("NEXT: Create Bitcoin Transaction");
        console.log("========================================");
        console.log("");
        console.log("Use the bitcoin_deposit.sh script:");
        console.log("");
        console.log("  ./script/flow/bitcoin_deposit.sh \\");
        console.log("    --amount", depositAmount, "\\");
        console.log("    --envelope", vm.toString(envelope), "\\");
        console.log("    --vault-spk", vm.toString(vm.envBytes("VAULT_SPK")));
        console.log("");
        console.log("Or manually with bitcoin-cli:");
        console.log("");
        console.log("1. Create raw transaction:");
        console.log(
            "   bitcoin-cli -testnet createrawtransaction '[{...}]' '{"
        );
        console.log(
            '     "<vault_address>":',
            depositAmount / 100000000.0,
            ","
        );
        console.log('     "data": "<envelope_hex>"');
        console.log("   }'");
        console.log("");
        console.log("2. Sign transaction:");
        console.log(
            "   bitcoin-cli -testnet signrawtransactionwithwallet <raw_tx>"
        );
        console.log("");
        console.log("3. Broadcast transaction:");
        console.log("   bitcoin-cli -testnet sendrawtransaction <signed_tx>");
        console.log("");
        console.log("4. Save TXID for Step 3!");
        console.log("");
    }
}
