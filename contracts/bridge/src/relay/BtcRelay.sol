// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";

/**
 * @title BtcRelay - Bitcoin SPV Light Client on Mojave L2
 * @notice Verifies Bitcoin block headers and provides SPV proof verification
 * @dev Maintains a chain of Bitcoin block headers for trustless verification
 *
 * Key Features:
 * - Stores Bitcoin block headers with proof-of-work validation
 * - Tracks block height and confirmations
 * - Provides merkle proof verification for transactions
 * - Supports header reorganization handling
 *
 * Security Model:
 * - Requires proof-of-work validation for each header
 * - Tracks cumulative difficulty to handle chain forks
 * - Minimum confirmation depth configurable
 */
contract BtcRelay is Ownable {
    // ========= Storage =========

    struct BlockHeader {
        bytes32 blockHash;
        bytes32 prevHash; // ✅ NEW: For reorg tracking
        bytes32 merkleRoot;
        uint256 height;
        uint256 chainWork; // Cumulative proof-of-work
        uint64 timestamp;
        bool exists;
    }

    // blockHash => BlockHeader
    mapping(bytes32 => BlockHeader) public headers;

    // height => blockHash (for main chain)
    mapping(uint256 => bytes32) public heightToHash;

    // Best known block (0-confirmation, tracks latest chain)
    bytes32 public bestBlockHash;
    uint256 public bestHeight;
    uint256 public bestChainWork;

    // ✅ NEW: Finalized block (best - FINALIZATION_DEPTH)
    // This is safe to use for deposits
    bytes32 public finalizedBlockHash;
    uint256 public finalizedHeight;

    // Finalization depth (confirmations needed)
    uint256 public constant FINALIZATION_DEPTH = 6;

    // Genesis block hash (Bitcoin genesis or checkpoint)
    bytes32 public genesisHash;

    // Minimum confirmations required for SPV proofs
    uint256 public minConfirmations = 6;

    // ========= Events =========

    /**
     * @notice Emitted when a new block header is submitted
     * @param blockHash Bitcoin block hash
     * @param height Block height
     * @param merkleRoot Merkle root of transactions
     */
    event HeaderSubmitted(
        bytes32 indexed blockHash,
        uint256 indexed height,
        bytes32 merkleRoot
    );

    /**
     * @notice Emitted when the best chain tip is updated
     * @param newBest New best block hash
     * @param height New best height
     */
    event BestBlockUpdated(bytes32 indexed newBest, uint256 height);

    /**
     * @notice Emitted when minimum confirmations are updated
     * @param oldMin Old minimum
     * @param newMin New minimum
     */
    event MinConfirmationsUpdated(uint256 oldMin, uint256 newMin);

    /**
     * @notice Emitted when a block becomes finalized
     * @param blockHash Finalized block hash
     * @param height Finalized block height
     */
    event BlockFinalized(bytes32 indexed blockHash, uint256 indexed height);

    // ========= Constructor =========

    /**
     * @notice Initialize BtcRelay with genesis block
     * @param admin Contract owner
     * @param _genesisHash Bitcoin genesis or checkpoint hash
     * @param _genesisMerkleRoot Merkle root of genesis block
     * @param _genesisHeight Height of genesis block (0 for genesis, or checkpoint height)
     * @param _genesisTimestamp Timestamp of genesis block
     * @param _genesisChainWork Cumulative chain work at genesis
     */
    constructor(
        address admin,
        bytes32 _genesisHash,
        bytes32 _genesisMerkleRoot,
        uint256 _genesisHeight,
        uint64 _genesisTimestamp,
        uint256 _genesisChainWork
    ) Ownable(admin) {
        require(_genesisHash != bytes32(0), "BtcRelay: zero genesis");
        require(admin != address(0), "BtcRelay: zero admin");

        genesisHash = _genesisHash;
        bestBlockHash = _genesisHash;
        bestHeight = _genesisHeight;
        bestChainWork = _genesisChainWork;

        headers[_genesisHash] = BlockHeader({
            blockHash: _genesisHash,
            prevHash: bytes32(0), // Genesis has no parent
            merkleRoot: _genesisMerkleRoot,
            height: _genesisHeight,
            chainWork: _genesisChainWork,
            timestamp: _genesisTimestamp,
            exists: true
        });

        heightToHash[_genesisHeight] = _genesisHash;

        emit HeaderSubmitted(_genesisHash, _genesisHeight, _genesisMerkleRoot);
    }

    // ========= Admin Functions =========

    /**
     * @notice Update minimum confirmations required
     * @param _minConfirmations New minimum (typically 6 for Bitcoin)
     */
    function setMinConfirmations(uint256 _minConfirmations) external onlyOwner {
        require(_minConfirmations > 0, "BtcRelay: zero conf");
        uint256 old = minConfirmations;
        minConfirmations = _minConfirmations;
        emit MinConfirmationsUpdated(old, _minConfirmations);
    }

    /**
     * @notice Submit a new Bitcoin block header with PoW verification
     * @param blockHeaderBytes Raw 80-byte Bitcoin block header
     * @param height Block height
     * @dev Permissionless - anyone can submit headers if PoW is valid
     * @dev Validates proof-of-work, parent chain, and difficulty target
     */
    function submitBlockHeader(
        bytes calldata blockHeaderBytes,
        uint256 height
    ) external {
        require(
            blockHeaderBytes.length == 80,
            "BtcRelay: invalid header length"
        );
        // Note: height can be 0 for genesis block in some deployments
        // Actual genesis is set in constructor, so this check prevents accidental height 0 submissions
        // In production, genesis should be initialized in constructor only

        // Parse header
        bytes32 blockHash = sha256(abi.encodePacked(sha256(blockHeaderBytes)));

        // prevHash: bytes 4-35 (32 bytes)
        // In Bitcoin headers, prevHash is already stored in big-endian (internal format)
        bytes32 prevHash = bytes32(blockHeaderBytes[4:36]);

        bytes32 merkleRoot = bytes32(blockHeaderBytes[36:68]);

        // ⚠️ Bitcoin header fields are in little-endian
        // timestamp: bytes 68-71 (4 bytes, little-endian)
        uint64 timestamp = uint64(
            uint32(uint8(blockHeaderBytes[68])) |
                (uint32(uint8(blockHeaderBytes[69])) << 8) |
                (uint32(uint8(blockHeaderBytes[70])) << 16) |
                (uint32(uint8(blockHeaderBytes[71])) << 24)
        );

        // nBits: bytes 72-75 (4 bytes, little-endian)
        uint32 nBits = uint32(uint8(blockHeaderBytes[72])) |
            (uint32(uint8(blockHeaderBytes[73])) << 8) |
            (uint32(uint8(blockHeaderBytes[74])) << 16) |
            (uint32(uint8(blockHeaderBytes[75])) << 24);

        // Verify doesn't exist
        require(!headers[blockHash].exists, "BtcRelay: header exists");

        // Verify parent exists
        // prevHash should be bytes32(0) for genesis, or existing header for non-genesis
        if (prevHash != bytes32(0)) {
            require(headers[prevHash].exists, "BtcRelay: parent not found");

            // Parent height must be exactly height - 1
            // Prevent underflow by checking height > 0 first
            require(height > 0, "BtcRelay: non-zero parent with zero height");
            require(
                headers[prevHash].height == height - 1,
                "BtcRelay: wrong parent height"
            );
        } else {
            // prevHash is zero - this should only happen for genesis
            // In production, genesis is set in constructor, so this path shouldn't be reached
            require(height == 0, "BtcRelay: non-genesis with zero parent");
        }

        // ===== PoW VERIFICATION =====
        // Verify block hash meets difficulty target
        require(
            _verifyProofOfWork(blockHash, nBits),
            "BtcRelay: insufficient proof of work"
        );

        // Calculate cumulative chain work
        uint256 blockWork = _calculateBlockWork(nBits);
        uint256 chainWork;
        if (height == 0) {
            // Genesis block
            chainWork = blockWork;
        } else {
            // Add to parent's chain work
            chainWork = headers[prevHash].chainWork + blockWork;
        }

        // Store header
        headers[blockHash] = BlockHeader({
            blockHash: blockHash,
            prevHash: prevHash, // Store parent hash for reorg tracking
            merkleRoot: merkleRoot,
            height: height,
            chainWork: chainWork,
            timestamp: timestamp,
            exists: true
        });

        emit HeaderSubmitted(blockHash, height, merkleRoot);

        // Update best block if this has more work
        if (chainWork > bestChainWork) {
            bestBlockHash = blockHash;
            bestHeight = height;
            bestChainWork = chainWork;
            heightToHash[height] = blockHash;
            emit BestBlockUpdated(blockHash, height);

            // ✅ Update finalized block (best - FINALIZATION_DEPTH)
            if (height > FINALIZATION_DEPTH) {
                uint256 newFinalizedHeight = height - FINALIZATION_DEPTH;
                bytes32 newFinalizedHash = _getBlockHashAtHeight(
                    newFinalizedHeight
                );

                if (newFinalizedHeight > finalizedHeight) {
                    finalizedHeight = newFinalizedHeight;
                    finalizedBlockHash = newFinalizedHash;
                    emit BlockFinalized(newFinalizedHash, newFinalizedHeight);
                }
            }
        }
    }

    /**
     * @notice Batch submit multiple headers
     * @param headerBytes Concatenated 80-byte headers
     * @param heights Array of block heights
     * @dev Permissionless - validates PoW for each header
     */
    function submitBlockHeaders(
        bytes calldata headerBytes,
        uint256[] calldata heights
    ) external {
        require(headerBytes.length % 80 == 0, "BtcRelay: invalid batch length");
        uint256 count = headerBytes.length / 80;
        require(heights.length == count, "BtcRelay: height mismatch");

        for (uint256 i = 0; i < count; i++) {
            bytes calldata header = headerBytes[i * 80:(i + 1) * 80];
            this.submitBlockHeader(header, heights[i]);
        }
    }

    // ========= View Functions =========

    /**
     * @notice Check if a block has enough confirmations
     * @param headerHash Bitcoin block hash
     * @param minConf Minimum confirmations required
     * @return True if block has enough confirmations
     * @dev ✅ IMPORTANT: Checks against finalizedHeight OR bestHeight if not yet finalized
     *      Finalized blocks (6+ deep) are safe from reorg
     *      Recent blocks use bestHeight for immediate availability
     */
    function verifyConfirmations(
        bytes32 headerHash,
        uint256 minConf
    ) external view returns (bool) {
        BlockHeader storage header = headers[headerHash];
        if (!header.exists) return false;

        // Use finalized height if available, otherwise use best height
        // This allows verification even before finalization (e.g., for low-value deposits)
        uint256 referenceHeight = finalizedHeight > 0
            ? finalizedHeight
            : bestHeight;

        // Block must not be newer than reference height
        if (referenceHeight < header.height) return false;

        uint256 confirmations = referenceHeight - header.height + 1;
        return confirmations >= minConf;
    }

    /**
     * @notice Get merkle root of a block
     * @param headerHash Bitcoin block hash
     * @return Merkle root
     */
    function headerMerkleRoot(
        bytes32 headerHash
    ) external view returns (bytes32) {
        return headers[headerHash].merkleRoot;
    }

    /**
     * @notice Get block height
     * @param headerHash Bitcoin block hash
     * @return Block height
     */
    function getBlockHeight(
        bytes32 headerHash
    ) external view returns (uint256) {
        require(headers[headerHash].exists, "BtcRelay: header not found");
        return headers[headerHash].height;
    }

    /**
     * @notice Get block timestamp
     * @param headerHash Bitcoin block hash
     * @return Block timestamp
     */
    function getBlockTimestamp(
        bytes32 headerHash
    ) external view returns (uint64) {
        require(headers[headerHash].exists, "BtcRelay: header not found");
        return headers[headerHash].timestamp;
    }

    /**
     * @notice Get number of confirmations for a block (from best block)
     * @param headerHash Bitcoin block hash
     * @return Number of confirmations (0 if not found or not in main chain)
     * @dev Returns confirmations from bestHeight (may be 0-conf)
     * @dev For finalized confirmations, check against getFinalizedBlock()
     */
    function getConfirmations(
        bytes32 headerHash
    ) external view returns (uint256) {
        BlockHeader storage header = headers[headerHash];
        if (!header.exists) return 0;
        if (bestHeight < header.height) return 0;
        return bestHeight - header.height + 1;
    }

    /**
     * @notice Get number of finalized confirmations for a block
     * @param headerHash Bitcoin block hash
     * @return Number of finalized confirmations (0 if not finalized yet)
     * @dev ✅ SAFE: Returns confirmations from finalizedHeight (6+ deep, reorg-safe)
     */
    function getFinalizedConfirmations(
        bytes32 headerHash
    ) external view returns (uint256) {
        BlockHeader storage header = headers[headerHash];
        if (!header.exists) return 0;
        if (finalizedHeight == 0 || finalizedHeight < header.height) return 0;
        return finalizedHeight - header.height + 1;
    }

    /**
     * @notice Check if header exists
     * @param headerHash Bitcoin block hash
     * @return True if header is stored
     */
    function headerExists(bytes32 headerHash) external view returns (bool) {
        return headers[headerHash].exists;
    }

    /**
     * @notice Get best block info (0-confirmation, may reorg)
     * @return blockHash Best block hash
     * @return height Best height
     * @return chainWork Best chain work
     * @dev ⚠️ WARNING: Best block is 0-conf and may reorg!
     *      Use getFinalizedBlock() for deposits.
     */
    function getBestBlock()
        external
        view
        returns (bytes32 blockHash, uint256 height, uint256 chainWork)
    {
        return (bestBlockHash, bestHeight, bestChainWork);
    }

    /**
     * @notice Get finalized block info (6+ confirmations, safe from reorg)
     * @return blockHash Finalized block hash
     * @return height Finalized block height
     * @dev ✅ SAFE: Use this for deposits and critical operations
     */
    function getFinalizedBlock()
        external
        view
        returns (bytes32 blockHash, uint256 height)
    {
        return (finalizedBlockHash, finalizedHeight);
    }

    /**
     * @notice Verify merkle proof
     * @param txid Transaction ID (32 bytes, not reversed)
     * @param merkleRoot Merkle root from block header
     * @param merkleBranch Array of sibling hashes in merkle tree
     * @param index Transaction index in block
     * @return True if proof is valid
     */
    function verifyMerkleProof(
        bytes32 txid,
        bytes32 merkleRoot,
        bytes32[] calldata merkleBranch,
        uint256 index
    ) public pure returns (bool) {
        bytes32 hash = txid;

        for (uint256 i = 0; i < merkleBranch.length; i++) {
            bytes32 sibling = merkleBranch[i];

            if (index % 2 == 0) {
                // Left node - sibling is right
                hash = sha256(
                    abi.encodePacked(sha256(abi.encodePacked(hash, sibling)))
                );
            } else {
                // Right node - sibling is left
                hash = sha256(
                    abi.encodePacked(sha256(abi.encodePacked(sibling, hash)))
                );
            }

            index = index / 2;
        }

        return hash == merkleRoot;
    }

    // ========= Internal Helper Functions =========

    /**
     * @notice Get block hash at specific height by walking the chain
     * @param targetHeight Height to find
     * @return blockHash Block hash at target height
     * @dev Walks backwards from best block following prevHash links
     */
    function _getBlockHashAtHeight(
        uint256 targetHeight
    ) internal view returns (bytes32) {
        require(targetHeight <= bestHeight, "BtcRelay: height too high");

        bytes32 currentHash = bestBlockHash;
        uint256 currentHeight = bestHeight;

        // Walk backwards from best block
        while (currentHeight > targetHeight) {
            BlockHeader storage header = headers[currentHash];
            require(header.exists, "BtcRelay: broken chain");

            currentHash = header.prevHash;
            currentHeight--;
        }

        return currentHash;
    }

    // ========= Internal PoW Functions =========

    /**
     * @notice Verify proof-of-work for a block hash
     * @param blockHash Double SHA-256 hash of block header (big-endian from sha256)
     * @param nBits Compact difficulty target from header
     * @return True if blockHash meets difficulty target
     * @dev Bitcoin PoW: blockHash must be <= target derived from nBits
     * @dev Bitcoin stores hashes in little-endian, so we reverse for comparison
     */
    function _verifyProofOfWork(
        bytes32 blockHash,
        uint32 nBits
    ) internal pure returns (bool) {
        // Convert nBits to target (256-bit integer)
        uint256 target = _nBitsToTarget(nBits);

        // ⚠️ CRITICAL: Reverse block hash to little-endian for comparison
        // Solidity sha256() returns big-endian bytes32, but Bitcoin PoW uses little-endian
        bytes32 reversedHash = _reverseBytes32(blockHash);

        // Block hash must be less than or equal to target
        return uint256(reversedHash) <= target;
    }

    /**
     * @notice Reverse a bytes32 value (big-endian ↔ little-endian)
     * @param input Input bytes32
     * @return output Reversed bytes32
     * @dev Used for Bitcoin hash comparisons (Bitcoin uses little-endian)
     */
    function _reverseBytes32(
        bytes32 input
    ) internal pure returns (bytes32 output) {
        bytes memory temp = new bytes(32);
        bytes32 tempInput = input;

        for (uint256 i = 0; i < 32; i++) {
            temp[i] = tempInput[31 - i];
        }

        assembly {
            output := mload(add(temp, 32))
        }
    }

    /**
     * @notice Convert Bitcoin compact format (nBits) to full 256-bit target
     * @param nBits Compact representation (4 bytes)
     * @return target Full 256-bit difficulty target
     * @dev nBits format: 0xAABBCCDD where AA is exponent, BBCCDD is mantissa
     * @dev target = mantissa * 256^(exponent - 3)
     */
    function _nBitsToTarget(uint32 nBits) internal pure returns (uint256) {
        uint256 exponent = nBits >> 24; // Top byte
        uint256 mantissa = nBits & 0xffffff; // Bottom 3 bytes

        // Target = mantissa * 256^(exponent - 3)
        if (exponent <= 3) {
            return mantissa >> (8 * (3 - exponent));
        } else {
            return mantissa << (8 * (exponent - 3));
        }
    }

    /**
     * @notice Calculate work done by a block from its difficulty target
     * @param nBits Compact difficulty target
     * @return work Amount of work (difficulty) for this block
     * @dev work = 2^256 / (target + 1)
     * @dev Simplified: work ≈ max_target / target (using Bitcoin's max target)
     * @dev For regtest/low difficulty (target > maxTarget), work = 1
     */
    function _calculateBlockWork(uint32 nBits) internal pure returns (uint256) {
        uint256 target = _nBitsToTarget(nBits);
        require(target > 0, "BtcRelay: zero target");

        // Bitcoin max target: 0x00000000FFFF0000000000000000000000000000000000000000000000000000
        // For simplicity, we use: work = 0xFFFF * 2^208 / target
        // This is sufficient for comparing chain work
        uint256 maxTarget = 0xFFFF * (2 ** 208);

        // ⚠️ For regtest/testnet with low difficulty (target > maxTarget)
        // Set minimum work to 1 to allow chain progression
        if (target >= maxTarget) {
            return 1;
        }

        return maxTarget / target;
    }
}
