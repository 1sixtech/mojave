// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import "forge-std/Script.sol";
import {BridgeGateway} from "../../src/BridgeGateway.sol";
import {BtcRelay} from "../../src/relay/BtcRelay.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/**
 * @title SubmitDepositSpvProof
 * @notice Operator submits SPV proof from REAL Bitcoin testnet transaction
 *
 * Prerequisites:
 * 1. Bitcoin transaction must be broadcast (from bitcoin_deposit.sh)
 * 2. Transaction must have MIN_CONFIRMATIONS confirmations
 * 3. You must have BITCOIN_DEPOSIT_TXID exported
 *
 * This script does NOT mock data - it expects REAL Bitcoin blockchain data:
 * - rawTx: From bitcoin-cli getrawtransaction
 * - blockHeader: From bitcoin-cli getblockheader
 * - merkleProof: Calculated merkle branch (requires indexer or manual calculation)
 *
 * Usage:
 *   export BITCOIN_DEPOSIT_TXID=<txid>
 *   export BITCOIN_BLOCK_HASH=<block_hash>
 *   export BITCOIN_RAW_TX=<hex>
 *   export BITCOIN_BLOCK_HEADER=<hex>
 *   export BITCOIN_MERKLE_PROOF=<hex>
 *   export BITCOIN_MERKLE_INDEX=<index>
 *   forge script script/flow/SubmitDepositSpvProof.s.sol --broadcast --rpc-url $MOJAVE_RPC_URL
 *
 * Getting Bitcoin Data:
 *
 * 1. Get raw transaction:
 *    bitcoin-cli -testnet getrawtransaction $TXID
 *
 * 2. Get transaction details to find block:
 *    bitcoin-cli -testnet getrawtransaction $TXID true | jq -r '.blockhash'
 *
 * 3. Get block header:
 *    bitcoin-cli -testnet getblockheader $BLOCK_HASH false
 *
 * 4. Calculate merkle proof (requires additional tooling):
 *    - Option A: Use bitcoin-cli getblock with verbose=2, manually calculate
 *    - Option B: Use external indexer/library (bitcoin-spv, etc.)
 *    - Option C: Use helper script (we'll create this next)
 */
contract SubmitDepositSpvProof is Script {
    BridgeGateway public bridge;
    BtcRelay public btcRelay;

    // Constants
    uint256 constant MIN_CONFIRMATIONS = 6; // BridgeGateway requires 6

    // Read from environment
    string MOJAVE_RPC_URL = vm.envString("MOJAVE_RPC_URL");
    address BRIDGE_ADDRESS = vm.envAddress("BRIDGE_ADDRESS");
    address BTC_RELAY_ADDRESS = vm.envAddress("BTC_RELAY_ADDRESS");
    address OPERATOR = vm.envAddress("OPERATOR");
    uint256 OPERATOR_KEY = vm.envUint("OPERATOR_KEY");

    // Bitcoin data from environment
    bytes32 BITCOIN_TXID;
    bytes32 BITCOIN_BLOCK_HASH;
    bytes BITCOIN_RAW_TX;
    bytes BITCOIN_BLOCK_HEADER;
    bytes BITCOIN_MERKLE_PROOF;
    uint256 BITCOIN_MERKLE_INDEX;

    function setUp() public {
        // Connect to Mojave L2
        vm.createSelectFork(MOJAVE_RPC_URL);

        bridge = BridgeGateway(payable(BRIDGE_ADDRESS));
        btcRelay = BtcRelay(BTC_RELAY_ADDRESS);

        console.log("========================================");
        console.log("Step 3: Operator Submits SPV Proof (REAL)");
        console.log("========================================");
        console.log("");
        console.log("Mojave L2 RPC:", MOJAVE_RPC_URL);
        console.log("BridgeGateway:", BRIDGE_ADDRESS);
        console.log("BtcRelay:", BTC_RELAY_ADDRESS);
        console.log("Operator:", OPERATOR);
        console.log("");

        // Load Bitcoin data from environment
        _loadBitcoinData();
    }

    function _loadBitcoinData() internal {
        // Check if all required environment variables are set
        try vm.envBytes32("BITCOIN_DEPOSIT_TXID") returns (bytes32 txid) {
            BITCOIN_TXID = txid;
        } catch {
            console.log("[ERROR] Missing BITCOIN_DEPOSIT_TXID");
            console.log("Export it from bitcoin_deposit.sh output");
            revert("Missing BITCOIN_DEPOSIT_TXID");
        }
        try vm.envBytes32("BITCOIN_BLOCK_HASH") returns (bytes32 blockHash) {
            BITCOIN_BLOCK_HASH = blockHash;
        } catch {
            console.log("[ERROR] Missing BITCOIN_BLOCK_HASH");
            console.log("Get it with:");
            console.log("  bitcoin-cli -testnet getrawtransaction");
            console.log(
                "    %s true | jq -r '.blockhash'",
                vm.toString(BITCOIN_TXID)
            );
            revert("Missing BITCOIN_BLOCK_HASH");
        }
        try vm.envBytes("BITCOIN_RAW_TX") returns (bytes memory rawTx) {
            BITCOIN_RAW_TX = rawTx;
        } catch {
            console.log("[ERROR] Missing BITCOIN_RAW_TX");
            console.log("Get it with:");
            console.log(
                "  bitcoin-cli -testnet getrawtransaction %s",
                vm.toString(BITCOIN_TXID)
            );
            revert("Missing BITCOIN_RAW_TX");
        }
        try vm.envBytes("BITCOIN_BLOCK_HEADER") returns (bytes memory header) {
            require(header.length == 80, "Invalid header length");
            BITCOIN_BLOCK_HEADER = header;
        } catch {
            console.log("[ERROR] Missing BITCOIN_BLOCK_HEADER");
            console.log("Get it with:");
            console.log(
                "  bitcoin-cli -testnet getblockheader %s false",
                vm.toString(BITCOIN_BLOCK_HASH)
            );
            revert("Missing BITCOIN_BLOCK_HEADER");
        }
        try vm.envBytes("BITCOIN_MERKLE_PROOF") returns (bytes memory proof) {
            BITCOIN_MERKLE_PROOF = proof;
        } catch {
            console.log("[ERROR] Missing BITCOIN_MERKLE_PROOF");
            console.log("Calculate merkle branch - see helper script:");
            console.log(
                "  ./script/flow/bitcoin_merkle.sh %s",
                vm.toString(BITCOIN_TXID)
            );
            revert("Missing BITCOIN_MERKLE_PROOF");
        }
        try vm.envUint("BITCOIN_MERKLE_INDEX") returns (uint256 index) {
            BITCOIN_MERKLE_INDEX = index;
        } catch {
            console.log("[ERROR] Missing BITCOIN_MERKLE_INDEX");
            console.log(
                "Get transaction index in block with bitcoin_merkle.sh"
            );
            revert("Missing BITCOIN_MERKLE_INDEX");
        }
        console.log("Bitcoin Data Loaded:");
        console.log("  TXID:", vm.toString(BITCOIN_TXID));
        console.log("  Block Hash:", vm.toString(BITCOIN_BLOCK_HASH));
        console.log("  Raw TX length:", BITCOIN_RAW_TX.length, "bytes");
        console.log(
            "  Block Header length:",
            BITCOIN_BLOCK_HEADER.length,
            "bytes"
        );
        console.log(
            "  Merkle Proof length:",
            BITCOIN_MERKLE_PROOF.length,
            "bytes"
        );
        console.log("  Merkle Index:", BITCOIN_MERKLE_INDEX);
        console.log("");
    }

    function run() public {
        // Step 1: Verify block header is in BtcRelay
        console.log("[Step 1] Verifying Bitcoin block header in BtcRelay...");
        console.log("");

        vm.startBroadcast(OPERATOR_KEY);

        // Extract merkle root from block header (bytes 36-67)
        bytes32 merkleRoot = _extractMerkleRoot();

        // BridgeGateway uses keccak256(header) as the header hash
        bytes32 headerHash = keccak256(BITCOIN_BLOCK_HEADER);

        console.log("  Header Hash (keccak256):", vm.toString(headerHash));
        console.log("  Merkle root:", vm.toString(merkleRoot));

        // Check if header exists in BtcRelay (Real BtcRelay should have it from Step 8)
        // Note: For Real BtcRelay, headers should already be submitted via submitBlockHeader
        // We skip the setHeader call as it doesn't exist in real BtcRelay
        console.log(
            "  Note: Using Real BtcRelay - headers submitted in previous step"
        );
        console.log("");

        // Step 2: Calculate TXID from raw TX
        console.log("[Step 2] Calculating TXID from raw TX...");
        console.log("");

        bytes32 txid = sha256(abi.encodePacked(sha256(BITCOIN_RAW_TX)));
        console.log("RPC TXID (display):", vm.toString(BITCOIN_TXID));
        console.log("Calculated TXID:", vm.toString(txid));
        console.log("");

        if (txid != BITCOIN_TXID) {
            console.log(
                "[NOTE] TXID mismatch is EXPECTED for SegWit transactions!"
            );
            console.log("  - bitcoin-cli shows witness-stripped TXID");
            console.log("  - Our rawTx includes witness data");
            console.log("  - BridgeGateway will verify with merkle proof");
            console.log("");
        }

        // Step 3: Extract deposit details from OP_RETURN
        console.log("[Step 3] Parsing deposit details...");
        console.log("");

        // Parse envelope from raw TX
        (
            address recipient,
            uint256 amountSats,
            bytes32 envelopeHash
        ) = _parseDepositDetails();

        uint256 balanceBefore = bridge.WBTC().balanceOf(recipient);

        console.log("Recipient:", recipient);
        console.log("Amount:", amountSats, "sats");
        console.log("Envelope hash:", vm.toString(envelopeHash));
        console.log("Balance before:", balanceBefore, "wBTC");
        console.log("");

        // Step 4: Submit SPV proof to BridgeGateway
        console.log("[Step 4] Submitting SPV proof to BridgeGateway...");
        console.log("");

        // Build merkle branch array (each node is 32 bytes)
        bytes32[] memory merkleBranch = _parseMerkleBranch();

        console.log("Merkle branch length:", merkleBranch.length);
        for (uint256 i = 0; i < merkleBranch.length; i++) {
            console.log("  [%d]:", i, vm.toString(merkleBranch[i]));
        }
        console.log("");

        // Build SpvProof struct
        BridgeGateway.SpvProof memory proof = BridgeGateway.SpvProof({
            rawTx: BITCOIN_RAW_TX,
            txid: BITCOIN_TXID, // Use witness-stripped TXID from environment
            merkleBranch: merkleBranch,
            index: uint32(BITCOIN_MERKLE_INDEX),
            header0: BITCOIN_BLOCK_HEADER,
            confirmHeaders: new bytes[](0) // Empty since we're using BtcRelay
        });

        try bridge.claimDepositSpv(recipient, amountSats, envelopeHash, proof) {
            console.log("SPV proof submitted successfully!");
        } catch Error(string memory reason) {
            console.log("[ERROR] SPV proof submission failed:");
            console.log("  Reason:", reason);
            revert(reason);
        } catch (bytes memory lowLevelData) {
            console.log("[ERROR] SPV proof submission failed (low-level)");
            console.logBytes(lowLevelData);
            revert("SPV proof submission failed");
        }
        vm.stopBroadcast();

        // Step 5: Verify balance increased
        uint256 balanceAfter = bridge.WBTC().balanceOf(recipient);
        uint256 minted = balanceAfter - balanceBefore;

        console.log("");
        console.log("========================================");
        console.log("SUCCESS!");
        console.log("========================================");
        console.log("");
        console.log("Deposit claimed via SPV proof");
        console.log("");
        console.log("Balance before:", balanceBefore, "wBTC");
        console.log("Balance after:", balanceAfter, "wBTC");
        console.log("Minted:", minted, "wBTC");
        console.log("");
        console.log("Bitcoin TXID:", vm.toString(BITCOIN_TXID));
        console.log("Mojave L2 Recipient:", recipient);
        console.log("");
    }

    function _extractMerkleRoot() internal view returns (bytes32) {
        // Extract merkle root from block header (bytes 36-67, little-endian)
        require(BITCOIN_BLOCK_HEADER.length == 80, "Invalid header length");

        bytes memory merkleBytes = new bytes(32);
        for (uint256 i = 0; i < 32; i++) {
            merkleBytes[i] = BITCOIN_BLOCK_HEADER[36 + i];
        }

        return bytes32(merkleBytes);
    }

    function _parseDepositDetails()
        internal
        view
        returns (address, uint256, bytes32)
    {
        // Parse Bitcoin transaction to extract:
        // 1. Vault output value (amount deposited)
        // 2. OP_RETURN envelope data (recipient, etc.)

        bytes memory rawTx = BITCOIN_RAW_TX;
        uint256 pos = 0;

        // Skip version (4 bytes)
        pos += 4;

        // Check for SegWit marker (0x00) and flag (0x01)
        bool isSegWit = false;
        if (
            rawTx.length > 5 &&
            uint8(rawTx[pos]) == 0x00 &&
            uint8(rawTx[pos + 1]) == 0x01
        ) {
            isSegWit = true;
            pos += 2; // Skip marker and flag
            console.log("[DEBUG] SegWit transaction detected");
        }

        // Read input count
        (uint256 inputCount, uint256 varintSize) = _readVarint(rawTx, pos);
        pos += varintSize;

        // Skip all inputs
        for (uint256 i = 0; i < inputCount; i++) {
            // Previous output (36 bytes)
            pos += 36;
            // Script length
            (uint256 scriptLen, uint256 vSize) = _readVarint(rawTx, pos);
            pos += vSize + scriptLen;
            // Sequence (4 bytes)
            pos += 4;
        }

        // Read output count
        (uint256 outputCount, uint256 outVarintSize) = _readVarint(rawTx, pos);
        pos += outVarintSize;

        // Parse outputs
        uint256 vaultAmount = 0;
        bytes memory envelopeData;

        for (uint256 i = 0; i < outputCount; i++) {
            // Read value (8 bytes, little-endian)
            uint256 value = 0;
            for (uint256 j = 0; j < 8; j++) {
                value |= uint256(uint8(rawTx[pos + j])) << (j * 8);
            }
            pos += 8;

            // Read script length
            (uint256 scriptLen, uint256 sSize) = _readVarint(rawTx, pos);
            pos += sSize;

            // Check if this is OP_RETURN (starts with 0x6a)
            if (scriptLen > 0 && uint8(rawTx[pos]) == 0x6a) {
                // This is OP_RETURN output
                // Skip OP_RETURN opcode (1 byte) and pushdata length
                uint256 dataStart = pos + 1;

                // Check for OP_PUSHDATA1 (0x4c)
                if (uint8(rawTx[dataStart]) == 0x4c) {
                    uint256 dataLen = uint8(rawTx[dataStart + 1]);
                    dataStart += 2;
                    envelopeData = _slice(rawTx, dataStart, dataLen);
                } else {
                    // Direct push (first byte is length)
                    uint256 dataLen = uint8(rawTx[dataStart]);
                    dataStart += 1;
                    envelopeData = _slice(rawTx, dataStart, dataLen);
                }

                pos += scriptLen;
            } else if (i == 0 && value > 0) {
                // First output with value is likely the vault output
                vaultAmount = value;
                pos += scriptLen;
            } else {
                // Skip other outputs
                pos += scriptLen;
            }
        }

        // Skip witness data if SegWit
        if (isSegWit) {
            console.log("[DEBUG] Skipping witness data...");
            for (uint256 i = 0; i < inputCount; i++) {
                (uint256 witnessCount, uint256 wSize) = _readVarint(rawTx, pos);
                pos += wSize;
                for (uint256 j = 0; j < witnessCount; j++) {
                    (uint256 witnessLen, uint256 wlSize) = _readVarint(
                        rawTx,
                        pos
                    );
                    pos += wlSize + witnessLen;
                }
            }
        }

        require(envelopeData.length > 0, "OP_RETURN envelope not found");
        require(vaultAmount > 0, "Vault output not found");

        // Parse envelope: opretTag (4) + chainId (32) + bridgeAddress (20) + recipient (20) + depositAmount (32)
        require(envelopeData.length == 108, "Invalid envelope length");

        // Extract recipient (bytes 56-75, after tag + chainId + bridge)
        address recipient = address(0);
        for (uint256 i = 0; i < 20; i++) {
            recipient = address(
                uint160(recipient) |
                    (uint160(uint8(envelopeData[56 + i])) <<
                        uint160(8 * (19 - i)))
            );
        }

        // Extract amount (bytes 76-107, last 32 bytes as uint256 big-endian)
        uint256 envelopeAmount = 0;
        for (uint256 i = 0; i < 32; i++) {
            envelopeAmount =
                (envelopeAmount << 8) |
                uint256(uint8(envelopeData[76 + i]));
        }

        // Verify amounts match
        require(
            vaultAmount == envelopeAmount,
            "Amount mismatch between vault and envelope"
        );

        // Calculate envelope hash
        bytes32 envelopeHash = keccak256(envelopeData);

        return (recipient, vaultAmount, envelopeHash);
    }

    function _slice(
        bytes memory data,
        uint256 start,
        uint256 length
    ) internal pure returns (bytes memory) {
        bytes memory result = new bytes(length);
        for (uint256 i = 0; i < length; i++) {
            result[i] = data[start + i];
        }
        return result;
    }

    function _readVarint(
        bytes memory data,
        uint256 pos
    ) internal pure returns (uint256 value, uint256 size) {
        uint8 firstByte = uint8(data[pos]);

        if (firstByte < 0xfd) {
            return (uint256(firstByte), 1);
        } else if (firstByte == 0xfd) {
            value =
                uint256(uint8(data[pos + 1])) |
                (uint256(uint8(data[pos + 2])) << 8);
            return (value, 3);
        } else if (firstByte == 0xfe) {
            value =
                uint256(uint8(data[pos + 1])) |
                (uint256(uint8(data[pos + 2])) << 8) |
                (uint256(uint8(data[pos + 3])) << 16) |
                (uint256(uint8(data[pos + 4])) << 24);
            return (value, 5);
        } else {
            // 0xff - not commonly used, but included for completeness
            for (uint256 i = 0; i < 8; i++) {
                value |= uint256(uint8(data[pos + 1 + i])) << (i * 8);
            }
            return (value, 9);
        }
    }

    function _parseMerkleBranch() internal view returns (bytes32[] memory) {
        // Parse merkle proof bytes into array of bytes32
        // Each merkle node is 32 bytes

        uint256 numNodes = BITCOIN_MERKLE_PROOF.length / 32;
        require(
            BITCOIN_MERKLE_PROOF.length % 32 == 0,
            "Invalid merkle proof length"
        );

        bytes32[] memory branch = new bytes32[](numNodes);

        for (uint256 i = 0; i < numNodes; i++) {
            bytes memory nodeBytes = new bytes(32);
            for (uint256 j = 0; j < 32; j++) {
                nodeBytes[j] = BITCOIN_MERKLE_PROOF[i * 32 + j];
            }
            branch[i] = bytes32(nodeBytes);
        }

        return branch;
    }
}
