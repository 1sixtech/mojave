// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {BridgeGateway, IMintBurnERC20} from "../src/BridgeGateway.sol";
import {MockWBTC} from "../src/mocks/MockWBTC.sol";
import {MockBtcRelay} from "../src/mocks/MockBtcRelay.sol";

contract BridgeGatewayTest is Test {
    BridgeGateway bridge;
    MockWBTC wbtc;
    MockBtcRelay relay;

    address user = address(0x7777);
    uint256 userPk = 0x7777;

    // operator keys
    uint256[5] opPks;
    address[] members;

    bytes vaultChangeSpk;
    bytes anchorSpk;
    bytes vaultSpk;

    function setUp() public {
        wbtc = new MockWBTC();
        relay = new MockBtcRelay();

        // simple P2TR-like spks (just byte equality in tests)
        vaultChangeSpk = hex"5120aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        anchorSpk = hex"5120bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        vaultSpk = hex"5120cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

        bridge = new BridgeGateway(
            address(wbtc),
            vaultChangeSpk,
            anchorSpk,
            true, // anchor required
            vaultSpk,
            bytes("MOJ_TAG"),
            address(relay)
        );
        // fund user
        wbtc.mint(user, 1_000_000_000); // 10 BTC in sats

        // operator set
        members = new address[](5);
        for (uint i = 0; i < 5; i++) {
            opPks[i] = uint256(keccak256(abi.encodePacked("op", i + 1)));
            members[i] = vm.addr(opPks[i]);
        }
        vm.prank(bridge.owner());
        bridge.createOperatorSet(1, members, 4, true);
    }

    function testWithdrawalFinalizeHappyPath() public {
        // First, register UTXO as collateral
        bytes32 txid = bytes32(uint256(0x1234));
        uint32 vout = 0;
        uint256 utxoAmount = 100_000_000; // 1 BTC
        bridge.registerCollateralUtxo(txid, vout, utxoAmount);

        uint256 amount = 50_000_000; // 0.5 BTC
        bytes
            memory destSpk = hex"5120dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
        uint64 deadline = uint64(block.timestamp + 1 days);

        // Prepare UTXO inputs
        BridgeGateway.UtxoInput[] memory utxos = new BridgeGateway.UtxoInput[](
            1
        );
        utxos[0] = BridgeGateway.UtxoInput({
            txid: txid,
            vout: vout,
            amount: utxoAmount
        });

        vm.startPrank(user);
        wbtc.approve(address(bridge), amount);
        bytes32 wid = bridge.requestWithdraw(amount, destSpk, deadline, utxos);
        vm.stopPrank();

        // pull state (Withdrawal has 12 fields, array not returned by getter)
        (
            address u,
            uint256 amt,
            ,
            uint64 ddl,
            bytes32 outputsHash,
            uint32 version,
            uint32 setId,
            ,
            ,
            ,

        ) = bridge.withdrawals(wid);
        assertEq(u, user);
        assertEq(amt, amount);
        assertEq(ddl, deadline);
        assertEq(setId, 1);

        // Build rawTx: 1 vin, 3 vout (dest, change, anchor)
        // casting to 'uint64' is safe because test amounts are bounded << 2^64
        // forge-lint: disable-next-line(unsafe-typecast)
        bytes memory rawTx = _buildRawTxLegacy(
            _toU64Array(uint64(amount), uint64(100_000_000), uint64(1_000)), // dest, change, anchor
            _toBytesArray(destSpk, vaultChangeSpk, anchorSpk)
        );

        uint64 expiry = uint64(block.timestamp + 1 hours);
        bytes32 digest = bridge.approvalDigestPublic(
            wid,
            outputsHash,
            version,
            expiry,
            setId
        );

        // collect 4 of 5 signatures: pick idx 0..3
        uint256 bitmap = 0;
        bytes[] memory sigs = new bytes[](4);
        for (uint i = 0; i < 4; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(opPks[i], digest);
            sigs[i] = abi.encodePacked(r, s, v);
            bitmap |= (uint256(1) << uint256(i));
        }

        // finalize
        bridge.finalizeByApprovals(
            wid,
            rawTx,
            outputsHash,
            version,
            setId,
            bitmap,
            sigs,
            expiry
        );

        // state is finalized and user tokens burned (bridge burned, user already transferred when locking)
        BridgeGateway.WState st;
        (, , , , , , , st, , , ) = bridge.withdrawals(wid);
        assertEq(uint8(st), uint8(BridgeGateway.WState.Finalized));
        // bridge had user's amount, and burned it
        // Optional: check event via expectEmit (omitted for brevity)
    }

    function testCancelAfterDeadline() public {
        // Register UTXO
        bytes32 txid = bytes32(uint256(0x5678));
        uint32 vout = 0;
        uint256 utxoAmount = 50_000; // 0.0005 BTC
        bridge.registerCollateralUtxo(txid, vout, utxoAmount);

        uint256 amount = 10_000;
        bytes memory destSpk = hex"51201111";
        uint64 deadline = uint64(block.timestamp + 1);

        BridgeGateway.UtxoInput[] memory utxos = new BridgeGateway.UtxoInput[](
            1
        );
        utxos[0] = BridgeGateway.UtxoInput({
            txid: txid,
            vout: vout,
            amount: utxoAmount
        });

        vm.startPrank(user);
        wbtc.approve(address(bridge), amount);
        bytes32 wid = bridge.requestWithdraw(amount, destSpk, deadline, utxos);
        vm.stopPrank();

        // move time
        vm.warp(block.timestamp + 2);
        uint256 before = wbtc.balanceOf(user);
        vm.prank(user);
        bridge.cancelWithdraw(wid);
        uint256 afterBal = wbtc.balanceOf(user);
        assertEq(afterBal, before + amount);
    }

    function testFinalizeRevertsOnOutputsMismatch() public {
        // Register UTXO
        bytes32 txid = bytes32(uint256(0x9abc));
        uint32 vout = 0;
        uint256 utxoAmount = 100_000; // 0.001 BTC
        bridge.registerCollateralUtxo(txid, vout, utxoAmount);

        uint256 amount = 12345;
        bytes
            memory destSpk = hex"5120eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"; // any
        uint64 deadline = uint64(block.timestamp + 1 days);

        BridgeGateway.UtxoInput[] memory utxos = new BridgeGateway.UtxoInput[](
            1
        );
        utxos[0] = BridgeGateway.UtxoInput({
            txid: txid,
            vout: vout,
            amount: utxoAmount
        });

        vm.startPrank(user);
        wbtc.approve(address(bridge), amount);
        bytes32 wid = bridge.requestWithdraw(amount, destSpk, deadline, utxos);
        vm.stopPrank();

        (
            ,
            ,
            ,
            ,
            bytes32 outputsHash,
            uint32 version,
            uint32 setId,
            ,
            ,
            ,

        ) = bridge.withdrawals(wid);

        // Build BAD rawTx (missing anchor)
        // casting to 'uint64' is safe because [explain why]
        // forge-lint: disable-next-line(unsafe-typecast)
        bytes memory rawTx = _buildRawTxLegacy(
            _toU64Array(uint64(amount), uint64(100_000)),
            _toBytesArray(destSpk, vaultChangeSpk)
        );

        uint64 expiry = uint64(block.timestamp + 1 hours);
        bytes32 digest = bridge.approvalDigestPublic(
            wid,
            outputsHash,
            version,
            expiry,
            setId
        );

        uint256 bitmap = 0;
        bytes[] memory sigs = new bytes[](4);
        for (uint i = 0; i < 4; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(opPks[i], digest);
            sigs[i] = abi.encodePacked(r, s, v);
            bitmap |= (uint256(1) << uint256(i));
        }

        vm.expectRevert(
            abi.encodeWithSelector(
                BridgeGateway.ErrOutputsMismatch.selector,
                wid
            )
        );
        bridge.finalizeByApprovals(
            wid,
            rawTx,
            outputsHash,
            version,
            setId,
            bitmap,
            sigs,
            expiry
        );
    }

    function testDepositSpvHappyPath() public {
        // craft a single-tx block: merkle root == txid
        address recipient = address(0xBEEF);
        uint256 amount = 50_000; // Use larger amount
        bytes memory opretTag = bytes("MOJ_TAG");
        bytes memory opretData = abi.encodePacked(
            opretTag,
            block.chainid,
            address(bridge),
            recipient,
            amount
        );
        bytes32 envelopeHash = keccak256(opretData);

        // rawTx with 1 vout to vaultSpk and 1 OP_RETURN(opretData)
        bytes[] memory spks = new bytes[](2);
        spks[0] = vaultSpk;
        spks[1] = bytes.concat(bytes1(0x6a), _pushData(opretData)); // OP_RETURN <data>
        uint64[] memory vals = new uint64[](2);
        vals[0] = uint64(amount);
        vals[1] = 0;

        bytes memory rawTx = _buildRawTxLegacy(vals, spks);
        bytes32 txid = _dblSha256(rawTx);

        // Build proper 80-byte Bitcoin header
        // Bitcoin headers store merkle root in little-endian format
        bytes32 merkleRootLE = _reverseBytes32(txid);
        bytes memory header80 = abi.encodePacked(
            uint32(0x20000000), // version (4 bytes)
            bytes32(0), // prev block hash (32 bytes)
            merkleRootLE, // merkle root in LE (32 bytes)
            uint32(block.timestamp), // timestamp (4 bytes)
            uint32(0x207fffff), // bits/difficulty (4 bytes)
            uint32(0) // nonce (4 bytes)
        );
        // Use Bitcoin's double-SHA256 for header hash (same as BridgeGateway.claimDepositSpv)
        bytes32 headerHash = sha256(abi.encodePacked(sha256(header80)));

        // relay setup
        relay.setHeader(headerHash, txid, 6);

        BridgeGateway.SpvProof memory proof;
        proof.rawTx = rawTx;
        proof.txid = txid;
        proof.merkleBranch = new bytes32[](0);
        proof.index = 0;
        proof.header0 = header80;
        proof.confirmHeaders = new bytes[](0);

        uint256 before = wbtc.balanceOf(recipient);
        bridge.claimDepositSpv(recipient, amount, envelopeHash, proof);
        uint256 afterBal = wbtc.balanceOf(recipient);
        assertEq(afterBal, before + amount);

        // duplicate should revert
        vm.expectRevert(
            abi.encodeWithSelector(
                BridgeGateway.ErrDuplicateDeposit.selector,
                txid, // proof.txid (big-endian)
                uint32(0) // voutIndex
            )
        );
        bridge.claimDepositSpv(recipient, amount, envelopeHash, proof);
    }

    // --- helpers ---

    function _toU64Array(
        uint64 a,
        uint64 b
    ) internal pure returns (uint64[] memory arr) {
        arr = new uint64[](2);
        arr[0] = a;
        arr[1] = b;
    }
    function _toU64Array(
        uint64 a,
        uint64 b,
        uint64 c
    ) internal pure returns (uint64[] memory arr) {
        arr = new uint64[](3);
        arr[0] = a;
        arr[1] = b;
        arr[2] = c;
    }
    function _toBytesArray(
        bytes memory a,
        bytes memory b
    ) internal pure returns (bytes[] memory arr) {
        arr = new bytes[](2);
        arr[0] = a;
        arr[1] = b;
    }
    function _toBytesArray(
        bytes memory a,
        bytes memory b,
        bytes memory c
    ) internal pure returns (bytes[] memory arr) {
        arr = new bytes[](3);
        arr[0] = a;
        arr[1] = b;
        arr[2] = c;
    }

    function _buildRawTxLegacy(
        uint64[] memory voutVals,
        bytes[] memory voutSpks
    ) internal pure returns (bytes memory) {
        require(voutVals.length == voutSpks.length, "len");
        bytes memory txb;
        txb = bytes.concat(txb, _le32(2)); // version=2
        txb = bytes.concat(txb, _varint(1)); // vin=1
        // dummy input (null outpoint)
        txb = bytes.concat(txb, new bytes(36)); // 32 txid + 4 vout
        txb = bytes.concat(txb, _varint(0)); // scriptSig len 0
        txb = bytes.concat(txb, hex"ffffffff"); // sequence
        // vouts
        txb = bytes.concat(txb, _varint(voutVals.length));
        for (uint i = 0; i < voutVals.length; i++) {
            txb = bytes.concat(txb, _le64(voutVals[i]));
            txb = bytes.concat(txb, _varint(voutSpks[i].length));
            txb = bytes.concat(txb, voutSpks[i]);
        }
        txb = bytes.concat(txb, _le32(0)); // locktime
        return txb;
    }

    function _varint(uint x) internal pure returns (bytes memory) {
        if (x < 0xfd) return bytes.concat(bytes1(uint8(x)));
        if (x <= 0xffff) return bytes.concat(bytes1(0xfd), _le16(uint16(x)));
        if (x <= 0xffffffff)
            return bytes.concat(bytes1(0xfe), _le32(uint32(x)));
        return bytes.concat(bytes1(0xff), _le64(uint64(x)));
    }
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
    function _dblSha256(bytes memory b) internal pure returns (bytes32) {
        return sha256(abi.encodePacked(sha256(b)));
    }
    function _pushData(bytes memory d) internal pure returns (bytes memory) {
        if (d.length <= 75) return bytes.concat(bytes1(uint8(d.length)), d);
        if (d.length <= 255)
            return bytes.concat(bytes1(0x4c), bytes1(uint8(d.length)), d); // OP_PUSHDATA1
        if (d.length <= 65535)
            return bytes.concat(bytes1(0x4d), _le16(uint16(d.length)), d); // OP_PUSHDATA2
        return bytes.concat(bytes1(0x4e), _le32(uint32(d.length)), d); // OP_PUSHDATA3
    }
    function _reverseBytes32(
        bytes32 input
    ) internal pure returns (bytes32 output) {
        bytes memory temp = new bytes(32);
        for (uint i = 0; i < 32; i++) {
            temp[i] = input[31 - i];
        }
        assembly {
            output := mload(add(temp, 32))
        }
    }
}
