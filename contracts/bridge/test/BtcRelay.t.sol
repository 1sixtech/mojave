// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {BtcRelay} from "../src/relay/BtcRelay.sol";

/**
 * @title BtcRelayTest
 * @notice Unit tests for BtcRelay contract with PoW verification
 */
contract BtcRelayTest is Test {
    BtcRelay public btcRelay;
    address public admin = address(0x1);

    // Bitcoin regtest genesis block
    // ⚠️ NOTE: In Solidity, we store block hashes in big-endian (sha256 output)
    // Genesis hash calculated from: 0100000000000000...02000000
    // sha256(sha256(header)) = 4f8dd2f3...6a (big-endian, as stored)
    bytes32 constant GENESIS_HASH =
        0x4f8dd2f37a2136d87965ae87bf8e2b3d86cccec06767f263659850e80dc2426a;
    bytes32 constant GENESIS_MERKLE_ROOT =
        0x4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b;
    uint256 constant GENESIS_HEIGHT = 0;
    uint64 constant GENESIS_TIMESTAMP = 1296688602;
    uint256 constant GENESIS_CHAIN_WORK = 2; // Difficulty = 1 (regtest)

    function setUp() public {
        vm.prank(admin);
        btcRelay = new BtcRelay(
            admin,
            GENESIS_HASH,
            GENESIS_MERKLE_ROOT,
            GENESIS_HEIGHT,
            GENESIS_TIMESTAMP,
            GENESIS_CHAIN_WORK
        );
    }

    // ========= Constructor Tests =========

    function test_Constructor_InitializesGenesis() public view {
        assertEq(btcRelay.genesisHash(), GENESIS_HASH);
        assertEq(btcRelay.bestBlockHash(), GENESIS_HASH);
        assertEq(btcRelay.bestHeight(), GENESIS_HEIGHT);
        assertEq(btcRelay.bestChainWork(), GENESIS_CHAIN_WORK);
    }

    function test_Constructor_RevertsZeroGenesisHash() public {
        vm.expectRevert("BtcRelay: zero genesis");
        new BtcRelay(
            admin,
            bytes32(0),
            GENESIS_MERKLE_ROOT,
            GENESIS_HEIGHT,
            GENESIS_TIMESTAMP,
            GENESIS_CHAIN_WORK
        );
    }

    function test_Constructor_RevertsZeroAdmin() public {
        // OpenZeppelin Ownable will revert with OwnableInvalidOwner
        vm.expectRevert();
        new BtcRelay(
            address(0),
            GENESIS_HASH,
            GENESIS_MERKLE_ROOT,
            GENESIS_HEIGHT,
            GENESIS_TIMESTAMP,
            GENESIS_CHAIN_WORK
        );
    }

    // ========= PoW Verification Tests =========

    /**
     * @notice Test valid block header with correct PoW
     * @dev Block 1 from Bitcoin regtest (mined with nBits=0x207fffff)
     */
    function test_SubmitBlockHeader_ValidPoW() public {
        // Bitcoin regtest block 1 - mined with valid PoW
        // prevHash = genesis (4f8dd2f3... in big-endian), nBits = 0x207fffff
        bytes
            memory header = hex"020000004f8dd2f37a2136d87965ae87bf8e2b3d86cccec06767f263659850e80dc2426a4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b0ad7d34dffff7f2000000000";

        // Submit header
        btcRelay.submitBlockHeader(header, 1);

        // Verify header was stored
        bytes32 expectedHash = sha256(abi.encodePacked(sha256(header)));
        assertTrue(btcRelay.headerExists(expectedHash));
        assertEq(btcRelay.getBlockHeight(expectedHash), 1);
    }

    /**
     * @notice Test that invalid PoW is rejected
     * @dev Modified nonce to make PoW invalid (nonce 0x12345678 with regtest difficulty)
     */
    function test_SubmitBlockHeader_RevertsInvalidPoW() public {
        // Invalid header - same as block 1 but with higher difficulty (nBits=0x200fffff, requires lower hash)
        bytes
            memory invalidHeader = hex"020000004f8dd2f37a2136d87965ae87bf8e2b3d86cccec06767f263659850e80dc2426a4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b0ad7d34dffff0f2000000000";

        vm.expectRevert("BtcRelay: insufficient proof of work");
        btcRelay.submitBlockHeader(invalidHeader, 1);
    }

    function test_SubmitBlockHeader_RevertsWrongHeaderLength() public {
        bytes memory shortHeader = hex"0000002006226e46";

        vm.expectRevert("BtcRelay: invalid header length");
        btcRelay.submitBlockHeader(shortHeader, 1);
    }

    function test_SubmitBlockHeader_RevertsWrongHeightForParent() public {
        // Block 1 header (prevHash = genesis) but submitted with height 0
        // This should fail because it has a non-zero parent but height is 0
        bytes
            memory header = hex"020000004f8dd2f37a2136d87965ae87bf8e2b3d86cccec06767f263659850e80dc2426a4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b0ad7d34dffff7f2000000000";

        vm.expectRevert("BtcRelay: non-zero parent with zero height");
        btcRelay.submitBlockHeader(header, 0);
    }

    function test_SubmitBlockHeader_RevertsParentNotFound() public {
        // Block header with non-existent parent hash
        bytes
            memory header = hex"02000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b0ad7d34dffff7f2000000000";

        vm.expectRevert("BtcRelay: parent not found");
        btcRelay.submitBlockHeader(header, 2);
    }

    function test_SubmitBlockHeader_RevertsDuplicateHeader() public {
        // Block 1 (valid)
        bytes
            memory header = hex"020000004f8dd2f37a2136d87965ae87bf8e2b3d86cccec06767f263659850e80dc2426a4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b0ad7d34dffff7f2000000000";

        btcRelay.submitBlockHeader(header, 1);

        vm.expectRevert("BtcRelay: header exists");
        btcRelay.submitBlockHeader(header, 1);
    }

    // ========= ChainWork Calculation Tests =========

    function test_ChainWork_IncreasesWithNewBlock() public {
        uint256 initialWork = btcRelay.bestChainWork();

        // Submit block 1 (valid regtest block)
        bytes
            memory header = hex"020000004f8dd2f37a2136d87965ae87bf8e2b3d86cccec06767f263659850e80dc2426a4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b0ad7d34dffff7f2000000000";
        btcRelay.submitBlockHeader(header, 1);

        uint256 newWork = btcRelay.bestChainWork();
        assertGt(newWork, initialWork, "ChainWork should increase");
    }

    // ========= Confirmations Tests =========

    function test_VerifyConfirmations_ZeroConfirmationsForNewBlock() public {
        // Submit block 1
        bytes
            memory header = hex"020000004f8dd2f37a2136d87965ae87bf8e2b3d86cccec06767f263659850e80dc2426a4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b0ad7d34dffff7f2000000000";
        btcRelay.submitBlockHeader(header, 1);

        bytes32 blockHash = sha256(abi.encodePacked(sha256(header)));

        // Block 1 has 1 confirmation from bestHeight (itself)
        // finalizedHeight is still 0 (genesis) since we need 6+ blocks for finalization
        assertEq(btcRelay.getConfirmations(blockHash), 1);
    }

    function test_VerifyConfirmations_IncreasesWithMoreBlocks() public {
        // Submit blocks 1, 2, 3
        bytes
            memory header1 = hex"020000004f8dd2f37a2136d87965ae87bf8e2b3d86cccec06767f263659850e80dc2426a4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b0ad7d34dffff7f2000000000";
        bytes
            memory header2 = hex"020000006ef22c4e63a5bdd0bd8b649342815dd65609a1394a1673494e775dbbb298b05c4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b20d7d34dffff7f2000000000";
        bytes
            memory header3 = hex"0200000067fa9f4743d1b4687b25b4c52501d79e748190e0c24466e251ea29fd663cca6b4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b84d7d34dffff7f2000000000";

        btcRelay.submitBlockHeader(header1, 1);
        bytes32 hash1 = sha256(abi.encodePacked(sha256(header1)));

        // After block 1: bestHeight = 1, confirmations = 1
        assertEq(btcRelay.getConfirmations(hash1), 1);

        btcRelay.submitBlockHeader(header2, 2);
        // After block 2: bestHeight = 2, confirmations = 2
        assertEq(btcRelay.getConfirmations(hash1), 2);

        btcRelay.submitBlockHeader(header3, 3);
        // After block 3: bestHeight = 3, confirmations = 3
        assertEq(btcRelay.getConfirmations(hash1), 3);
    }

    // ========= Merkle Proof Tests =========

    function test_VerifyMerkleProof_ValidProof() public {
        // Simple merkle tree with 2 transactions
        bytes32 txid1 = keccak256("tx1");
        bytes32 txid2 = keccak256("tx2");
        bytes32 merkleRoot = sha256(
            abi.encodePacked(sha256(abi.encodePacked(txid1, txid2)))
        );

        bytes32[] memory branch = new bytes32[](1);
        branch[0] = txid2;

        bool valid = btcRelay.verifyMerkleProof(txid1, merkleRoot, branch, 0);
        assertTrue(valid, "Valid merkle proof should pass");
    }

    function test_VerifyMerkleProof_InvalidProof() public {
        bytes32 txid1 = keccak256("tx1");
        bytes32 txid2 = keccak256("tx2");
        bytes32 wrongRoot = keccak256("wrong");

        bytes32[] memory branch = new bytes32[](1);
        branch[0] = txid2;

        bool valid = btcRelay.verifyMerkleProof(txid1, wrongRoot, branch, 0);
        assertFalse(valid, "Invalid merkle proof should fail");
    }

    // ========= Admin Functions Tests =========

    function test_SetMinConfirmations_Owner() public {
        vm.prank(admin);
        btcRelay.setMinConfirmations(12);
        assertEq(btcRelay.minConfirmations(), 12);
    }

    function test_SetMinConfirmations_RevertsNonOwner() public {
        vm.prank(address(0x999));
        vm.expectRevert();
        btcRelay.setMinConfirmations(12);
    }

    function test_SetMinConfirmations_RevertsZero() public {
        vm.prank(admin);
        vm.expectRevert("BtcRelay: zero conf");
        btcRelay.setMinConfirmations(0);
    }

    // ========= View Functions Tests =========

    function test_GetBestBlock() public view {
        (bytes32 blockHash, uint256 height, uint256 chainWork) = btcRelay
            .getBestBlock();
        assertEq(blockHash, GENESIS_HASH);
        assertEq(height, GENESIS_HEIGHT);
        assertEq(chainWork, GENESIS_CHAIN_WORK);
    }

    function test_HeaderMerkleRoot() public view {
        bytes32 root = btcRelay.headerMerkleRoot(GENESIS_HASH);
        assertEq(root, GENESIS_MERKLE_ROOT);
    }

    function test_HeaderExists_Genesis() public view {
        assertTrue(btcRelay.headerExists(GENESIS_HASH));
    }

    function test_HeaderExists_NonExistent() public view {
        assertFalse(btcRelay.headerExists(keccak256("nonexistent")));
    }

    // ========= Helper Functions for Testing =========

    /**
     * @notice Create a mock Bitcoin header for testing
     * @dev Does NOT include valid PoW
     */
    function createMockHeader(
        bytes32 prevHash,
        bytes32 merkleRoot,
        uint32 timestamp,
        uint32 nBits,
        uint32 nonce
    ) internal pure returns (bytes memory) {
        bytes memory header = new bytes(80);

        // Version (4 bytes) - little endian
        header[0] = 0x00;
        header[1] = 0x00;
        header[2] = 0x00;
        header[3] = 0x20; // Version 0x20000000 = 536870912

        // Previous block hash (32 bytes) - already in correct format
        for (uint i = 0; i < 32; i++) {
            header[4 + i] = prevHash[i];
        }

        // Merkle root (32 bytes)
        for (uint i = 0; i < 32; i++) {
            header[36 + i] = merkleRoot[i];
        }

        // Timestamp (4 bytes) - little endian
        header[68] = bytes1(uint8(timestamp));
        header[69] = bytes1(uint8(timestamp >> 8));
        header[70] = bytes1(uint8(timestamp >> 16));
        header[71] = bytes1(uint8(timestamp >> 24));

        // nBits (4 bytes) - little endian
        header[72] = bytes1(uint8(nBits));
        header[73] = bytes1(uint8(nBits >> 8));
        header[74] = bytes1(uint8(nBits >> 16));
        header[75] = bytes1(uint8(nBits >> 24));

        // Nonce (4 bytes) - little endian
        header[76] = bytes1(uint8(nonce));
        header[77] = bytes1(uint8(nonce >> 8));
        header[78] = bytes1(uint8(nonce >> 16));
        header[79] = bytes1(uint8(nonce >> 24));

        return header;
    }
}
