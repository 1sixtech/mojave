// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";

/**
 * @title Simple Gas Cost Measurement
 * @notice Measures gas cost of storage operations
 */
contract MeasureStorageGas is Script {
    // Storage mappings to test
    mapping(bytes32 => bool) public testSpent;
    mapping(bytes32 => uint8) public testSource;

    event UtxoRegistered(
        bytes32 indexed utxoId,
        bytes32 indexed txid,
        uint32 vout,
        uint256 amount,
        uint8 indexed source,
        uint256 timestamp
    );

    function run() external {
        console.log("=== UTXO Storage Gas Cost Analysis ===");
        console.log("");

        vm.startBroadcast();

        // Test 1: Measure storage cost
        measureStorageCost();

        // Test 2: Measure event cost
        measureEventCost();

        // Test 3: Combined cost
        measureCombinedCost();

        vm.stopBroadcast();
    }

    function measureStorageCost() internal {
        console.log("--- Test 1: Storage Only ---");

        bytes32 utxoId = keccak256("test_utxo_1");

        uint256 gasBefore = gasleft();
        testSpent[utxoId] = false; // SSTORE cold
        testSource[utxoId] = 1; // SSTORE cold
        uint256 gasUsed = gasBefore - gasleft();

        console.log("Two SSTORE operations:", gasUsed, "gas");
        console.log("Expected: ~40,000 gas (20k per SSTORE)");
        console.log("");
    }

    function measureEventCost() internal {
        console.log("--- Test 2: Event Only ---");

        bytes32 utxoId = keccak256("test_utxo_2");
        bytes32 txid = keccak256("test_tx");

        uint256 gasBefore = gasleft();
        emit UtxoRegistered(utxoId, txid, 0, 100000000, 1, block.timestamp);
        uint256 gasUsed = gasBefore - gasleft();

        console.log("Event emission (LOG4):", gasUsed, "gas");
        console.log("Expected: ~3,600 gas");
        console.log("");
    }

    function measureCombinedCost() internal {
        console.log("--- Test 3: Storage + Event (Current Implementation) ---");

        bytes32 utxoId = keccak256("test_utxo_3");
        bytes32 txid = keccak256("test_tx_3");

        uint256 gasBefore = gasleft();

        // This is what claimDepositSpv() does
        testSpent[utxoId] = false;
        testSource[utxoId] = 1;
        emit UtxoRegistered(utxoId, txid, 0, 100000000, 1, block.timestamp);

        uint256 gasUsed = gasBefore - gasleft();

        console.log("Total cost:", gasUsed, "gas");
        console.log("Expected: ~43,600 gas");
        console.log("");
        console.log("Breakdown:");
        console.log("  - utxoSpent storage:  ~20,000 gas");
        console.log("  - utxoSource storage: ~20,000 gas");
        console.log("  - Event emission:      ~3,600 gas");
        console.log("");
    }

    function measureValidationCost() internal view {
        console.log("--- Test 4: Validation Cost (Withdrawal) ---");

        bytes32 utxoId = keccak256("test_utxo_1");

        uint256 gasBefore = gasleft();

        // What requestWithdraw() does per UTXO
        bool isSpent = testSpent[utxoId];
        require(!isSpent, "Already spent");

        uint8 source = testSource[utxoId];
        require(source == 1 || source == 2, "Invalid source");

        uint256 gasUsed = gasBefore - gasleft();

        console.log("Per-UTXO validation:", gasUsed, "gas");
        console.log("For 5 UTXOs:", gasUsed * 5, "gas");
        console.log("");
    }
}
