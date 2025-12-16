// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

contract MockBtcRelay {
    mapping(bytes32 => bytes32) public headerMerkleRoots;
    mapping(bytes32 => uint256) public headerHeights;
    uint256 public bestHeight;
    bytes32 public bestHeader;

    function setMerkleRoot(bytes32 headerHash, bytes32 merkleRoot) external {
        headerMerkleRoots[headerHash] = merkleRoot;
    }

    function setHeaderHeight(bytes32 headerHash, uint256 height) external {
        headerHeights[headerHash] = height;
        if (height > bestHeight) {
            bestHeight = height;
            bestHeader = headerHash;
        }
    }

    // Helper for tests: set header with merkle root and confirmations
    function setHeader(
        bytes32 headerHash,
        bytes32 merkleRoot,
        uint256 confirmations
    ) external {
        headerMerkleRoots[headerHash] = merkleRoot;
        // Set height such that it has the specified confirmations
        uint256 height = bestHeight + 1;
        headerHeights[headerHash] = height;
        bestHeight = height + confirmations - 1;
        bestHeader = headerHash;
    }

    function verifyConfirmations(
        bytes32 headerHash,
        uint256 minConf
    ) external view returns (bool) {
        uint256 headerHeight = headerHeights[headerHash];
        if (headerHeight == 0) return false;
        if (bestHeight < headerHeight) return false;
        uint256 confirmations = bestHeight - headerHeight + 1;
        return confirmations >= minConf;
    }

    function headerMerkleRoot(
        bytes32 headerHash
    ) external view returns (bytes32) {
        return headerMerkleRoots[headerHash];
    }
}
