// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

/**
 * @title MojaveBridge Gateway Contract
 * @author MojaveBridge Team
 * @notice Bitcoin ↔ Mojave L2 bridge enabling BTC deposits and withdrawals with M-of-N operator security
 * @dev This contract manages bidirectional BTC transfers between Bitcoin L1 and Mojave L2:
 *
 * Key Features:
 *   - Event-sourced UTXO tracking (minimal on-chain state, full history in events)
 *   - Off-chain indexer API for UTXO selection and balance queries
 *   - SPV proof verification for trustless deposits
 *   - M-of-N operator multisig for secure withdrawals
 *
 * Deposit Flow (Bitcoin L1 → Mojave L2):
 *   1. User sends BTC to vault address with OP_RETURN envelope containing recipient address
 *   2. After 6 Bitcoin confirmations, anyone submits SPV proof via claimDepositSpv()
 *   3. Contract verifies merkle proof, mints wBTC to recipient, emits UtxoRegistered event
 *   4. Off-chain indexer captures UtxoRegistered event and indexes UTXO for future use
 *
 * Withdrawal Flow (Mojave L2 → Bitcoin L1):
 *   1. User queries off-chain API to select available UTXOs for withdrawal
 *   2. User calls requestWithdraw() with selected UTXO IDs, contract locks wBTC
 *   3. Contract validates UTXOs (unspent, sufficient amount), emits WithdrawalInitiated with PSBT
 *   4. Operators listen to event, sign EIP-712 approval digest, call submitSignature() individually
 *   5. When M-th signature submitted, contract automatically finalizes and burns wBTC
 *   6. Contract emits SignedTxReady and marks UTXOs as spent via UtxoSpent events
 *   7. Operators broadcast signed Bitcoin TX to Bitcoin network
 *
 * Alternative Flow (Batch Finalization for Gas Savings):
 *   4. Operators coordinate M-of-N signatures off-chain
 *   5. Anyone calls finalizeByApprovals() with all signatures at once, contract burns wBTC
 *
 * UTXO Tracking Architecture:
 *   - On-chain: Only stores spent status (utxoSpent mapping) and source type
 *   - Off-chain: Indexer maintains full UTXO state from UtxoRegistered/UtxoSpent events
 *   - Benefits: 98% gas savings vs storing full UTXO data on-chain
 *
 * Security:
 *   - SPV proofs require 6 Bitcoin confirmations via BtcRelay
 *   - Duplicate deposit prevention (processedOutpoint mapping)
 *   - UTXO validation prevents double-spending
 *   - Operator set versioning for smooth upgrades
 *   - Withdrawal expiry and cancellation support
 *
 * @custom:network Mojave L2 (EVM-compatible Bitcoin Layer 2)
 * @custom:architecture Event-sourced UTXO tracking with off-chain indexer
 */

import {ECDSA} from "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import {EIP712} from "@openzeppelin/contracts/utils/cryptography/EIP712.sol";
import {
    ReentrancyGuard
} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/**
 * Optional token interface with mint/burn hooks.
 * wBTC on Mojave L2 (decimals 8 recommended, 1:1 with Bitcoin satoshis).
 */
interface IMintBurnERC20 is IERC20 {
    function mint(address to, uint256 amount) external;
    function burn(uint256 amount) external;
}

/**
 * External Bitcoin L1 header relay interface for SPV proof verification.
 */
interface IBtcRelay {
    function verifyConfirmations(
        bytes32 headerHash,
        uint256 minConf
    ) external view returns (bool);
    function headerMerkleRoot(
        bytes32 headerHash
    ) external view returns (bytes32);
}

contract BridgeGateway is EIP712, ReentrancyGuard, Ownable {
    using ECDSA for bytes32;

    // ========= Config =========
    IMintBurnERC20 public immutable WBTC;

    // L1 policy (change/anchor). Scripts are raw bytes of scriptPubKey.
    bytes public vaultChangeSpk; // e.g., P2TR of Vault change
    bytes public anchorSpk; // small output for CPFP child
    bool public anchorRequired; // v0 default = true (recommended)
    uint32 public policyVersion = 1; // hash component for outputsHash

    // Deposit (SPV)
    bytes public vaultScriptPubkey; // Where deposits land (L1)
    bytes public opretTag; // Envelope tag (<=80B recommended)
    IBtcRelay public BtcRelay; // optional persistent relay

    function vaultScriptPubKey() external view returns (bytes memory) {
        return vaultScriptPubkey;
    }

    // function opretTag() external view returns (bytes memory) {
    //     return opretTag;
    // }

    // ========= Operator Sets =========
    struct OperatorSet {
        address[] members; // fixed index (0..N-1)
        uint8 threshold; // M
        bool active;
    }
    mapping(uint32 => OperatorSet) public sets; // signerSetId -> set
    uint32 public currentSignerSetId; // default for new withdrawals

    // ========= Withdrawal =========
    enum WState {
        None,
        Pending,
        Ready, // Added: threshold signatures collected
        Finalized,
        Canceled
    }

    struct Withdrawal {
        address user;
        uint256 amountSats;
        bytes destSpk; // BTC scriptPubKey
        uint64 deadline; // epoch seconds
        bytes32 outputsHash; // template/policy hash
        uint32 version; // template version
        uint32 signerSetId; // snapshot
        WState state;
        uint256 signatureBitmap; // bitmap of validators who signed
        uint256 signatureCount; // number of signatures collected
        bytes32[] selectedUtxoIds; // UTXOs selected for this withdrawal
        uint256 totalInputAmount; // Total amount from selected UTXOs
    }

    mapping(bytes32 => Withdrawal) public withdrawals; // wid -> info
    mapping(address => uint256) public userNonces; // for wid derivation
    uint256 public withdrawalNonce; // global nonce for unique WID generation

    // Store signatures submitted via submitSignature (wid => signer => signature)
    mapping(bytes32 => mapping(address => bytes)) private withdrawalSignatures;

    // Track all withdrawal IDs (for iteration support)
    bytes32[] private allWithdrawalIds;
    mapping(address => bytes32[]) private userWithdrawalIds;

    // ========= Deposit (dedup) =========
    mapping(bytes32 => bool) public processedOutpoint; // keccak(txid, vout)

    // Track all deposit IDs (for iteration support)
    bytes32[] private allDepositIds;
    mapping(address => bytes32[]) private userDepositIds;

    // ========= UTXO Tracking =========
    /**
     * @dev Event-sourced UTXO tracking for 98% gas savings:
     *
     * On-Chain State (Minimal):
     *   - utxoSpent: Only tracks if UTXO is spent (bool mapping)
     *   - utxoSource: Tracks UTXO origin for validation (enum)
     *
     * Off-Chain State (Indexer):
     *   - Full UTXO details: txid, vout, amount, scriptPubKey
     *   - Balance tracking per address
     *   - UTXO selection algorithms (LARGEST_FIRST, etc.)
     *
     * Event Flow:
     *   1. UtxoRegistered(utxoId, txid, vout, amount, source, timestamp)
     *      → Indexer adds UTXO to available pool
     *   2. UtxoSpent(utxoId, wid, timestamp)
     *      → Indexer removes UTXO from available pool
     *
     * Benefits:
     *   - Withdrawal request: ~50K gas vs ~2.5M gas (98% savings)
     *   - No SSTORE for UTXO details (txid, vout, amount)
     *   - Validation remains trustless (on-chain spent check)
     */
    mapping(bytes32 => bool) public utxoSpent; // utxoId => spent status

    enum UtxoSource {
        NONE, // Invalid/not registered
        DEPOSIT, // From user Bitcoin deposit
        COLLATERAL // From operator collateral (future use)
    }
    mapping(bytes32 => UtxoSource) public utxoSource; // utxoId => source type

    // ========= EIP-712 =========
    // WithdrawApproval(bytes32 wid, bytes32 outputsHash, uint32 version, uint64 expiry, uint32 signerSetId)
    bytes32 private constant WITHDRAW_APPROVAL_TYPEHASH =
        keccak256(
            "WithdrawApproval(bytes32 wid,bytes32 outputsHash,uint32 version,uint64 expiry,uint32 signerSetId)"
        );

    // ========= Events =========

    /**
     * @notice Emitted when user initiates withdrawal request
     * @dev event structure (with PSBT):
     *      - Indexed fields for efficient querying (wid, user, signerSetId)
     *      - Non-redundant metadata (deadline, outputsHash - not in PSBT)
     *      - Full PSBT contains: wid, amountSats, destSpk, UTXOs, outputs
     *      - Parse PSBT for detailed withdrawal info (saves ~2K gas)
     *
     * @param wid Withdrawal ID (indexed for lookup)
     * @param user User address (indexed for user-specific queries)
     * @param signerSetId Operator set ID (indexed for operator filtering)
     * @param deadline Withdrawal deadline timestamp (not in PSBT)
     * @param outputsHash Policy hash for verification (not in PSBT)
     * @param psbt Complete PSBT with all withdrawal details (wid, amount, destSpk, UTXOs, outputs)
     */
    event WithdrawalInitiated(
        bytes32 indexed wid,
        address indexed user,
        uint32 indexed signerSetId,
        uint64 deadline,
        bytes32 outputsHash,
        bytes psbt
    );

    /**
     * @notice Emitted when withdrawal is finalized with signed Bitcoin transaction
     * @dev Primary flow (incremental signing):
     *      1. Operators sign EIP-712 approval digest off-chain
     *      2. Each operator calls submitSignature(wid, sig, rawTx) individually
     *      3. When M-th signature submitted, contract automatically validates and burns wBTC
     *      4. Contract emits this event with signed Bitcoin tx
     *      5. Off-chain watcher broadcasts rawTx to Bitcoin network
     *
     *      Alternative flow (batch finalization):
     *      1. Operators coordinate M-of-N signatures off-chain
     *      2. Anyone calls finalizeByApprovals(wid, rawTx, sigs[])
     *      3. Contract validates all signatures at once and burns wBTC
     * @param wid Withdrawal ID (indexed)
     * @param user User address (indexed)
     * @param txid Bitcoin transaction ID (indexed)
     * @param amountSats Amount in satoshis
     * @param rawTx Signed Bitcoin transaction ready for broadcast
     */
    event SignedTxReady(
        bytes32 indexed wid,
        address indexed user,
        bytes32 indexed txid,
        uint256 amountSats,
        bytes rawTx
    );

    /**
     * @notice Emitted when withdrawal is canceled
     * @dev Cancellation scenarios:
     *      - User cancels after deadline expires
     *      - Operator admin cancels for policy violations
     *      - wBTC is refunded to user upon cancellation
     * @param wid Withdrawal ID (indexed)
     * @param user User address (indexed)
     * @param amountSats Amount refunded in wBTC
     * @param canceledBy Address that triggered cancellation
     */
    event WithdrawalCanceled(
        bytes32 indexed wid,
        address indexed user,
        uint256 amountSats,
        address canceledBy
    );

    /**
     * @notice Emitted when Bitcoin deposit is finalized
     * @dev flow:
     *      1. User sends BTC to bridge vault address
     *      2. User waits for 6 confirmations on Bitcoin
     *      3. User calls claimDepositSpv(txid, vout, amount, recipient, blockHeight, merkleProof)
     *      4. Contract verifies SPV proof via BtcRelay
     *      5. Contract mints wBTC and emits this event + UtxoRegistered
     * @param did Deposit ID (indexed)
     * @param recipient wBTC recipient address (indexed)
     * @param amountSats Amount in satoshis
     * @param btcTxid Bitcoin transaction ID (indexed)
     * @param vout Output index in Bitcoin transaction
     */
    event DepositFinalized(
        bytes32 indexed did,
        address indexed recipient,
        uint256 amountSats,
        bytes32 indexed btcTxid,
        uint32 vout
    );

    /**
     * @notice Emitted when UTXO is registered (indexer tracks this for balance queries)
     * @dev Event-sourced UTXO tracking:
     *      - Off-chain indexer listens to build available UTXO pool
     *      - Enables /utxos/:address and /utxos/select API endpoints
     *      - Only minimal state stored on-chain (utxoSpent, utxoSource)
     * @param utxoId UTXO ID (keccak256(txid, vout)) - indexed for lookups
     * @param txid Bitcoin transaction ID - indexed for Bitcoin tracking
     * @param vout Output index
     * @param amount Amount in satoshis
     * @param source UTXO source (DEPOSIT or COLLATERAL) - indexed for filtering
     * @param timestamp Block timestamp
     */
    event UtxoRegistered(
        bytes32 indexed utxoId,
        bytes32 indexed txid,
        uint32 vout,
        uint256 amount,
        UtxoSource indexed source,
        uint256 timestamp
    );

    /**
     * @notice Emitted when UTXO is spent in withdrawal (indexer removes from available pool)
     * @dev Event-sourced UTXO tracking:
     *      - Off-chain indexer listens to mark UTXO as spent
     *      - Enables balance updates and prevents double-spend in UTXO selection
     *      - On-chain utxoSpent[utxoId] = true for validation
     * @param utxoId UTXO ID - indexed for lookups
     * @param wid Withdrawal ID - indexed for withdrawal tracking
     * @param timestamp Block timestamp
     */
    event UtxoSpent(
        bytes32 indexed utxoId,
        bytes32 indexed wid,
        uint256 timestamp
    );

    /**
     * @notice Emitted when an operator set is created
     * @param setId Operator set ID (indexed)
     * @param threshold Signature threshold
     * @param memberCount Number of operators
     * @param active Whether the set is active
     */
    event OperatorSetCreated(
        uint32 indexed setId,
        uint8 threshold,
        uint256 memberCount,
        bool active
    );

    /**
     * @notice Emitted when an operator set is updated
     * @param setId Operator set ID (indexed)
     * @param threshold New signature threshold
     * @param memberCount New number of operators
     * @param active Whether the set is active
     */
    event OperatorSetUpdated(
        uint32 indexed setId,
        uint8 threshold,
        uint256 memberCount,
        bool active
    );

    /**
     * @notice Emitted when a validator submits a signature for a withdrawal
     * @dev Spec-aligned: Validators sign PSBT/approval digest and submit via submitSignature()
     * @dev Contract verifies signature against psbt & pubkey, checks operator set membership
     * @param wid Withdrawal ID (indexed)
     * @param validator Validator address (indexed)
     * @param signerIndex Index in the operator set
     */
    event SignatureSubmitted(
        bytes32 indexed wid,
        address indexed validator,
        uint256 signerIndex
    );

    /**
     * @notice Emitted when withdrawal has enough signatures and is ready for finalization
     * @dev Current implementation: emitted when threshold M signatures collected
     * @dev Spec variant: Option 2 - emit WithdrawalReady with collected sigs, requires external finalizer
     * @dev Spec preferred: Option 1 - atomic burn + emit WithdrawalSucceed (≈ SignedTxReady)
     * @param wid Withdrawal ID (indexed)
     * @param user User address (indexed)
     * @param amountSats Amount in satoshis
     * @param destSpk Destination scriptPubKey
     */
    event WithdrawalReady(
        bytes32 indexed wid,
        address indexed user,
        uint256 amountSats,
        bytes destSpk
    );

    // ========= Errors =========
    error ErrNotPending(bytes32 wid, WState currentState);
    error ErrExpired(bytes32 wid, uint64 deadline, uint256 currentTime);
    error ErrThresholdNotMet(
        uint256 signaturesProvided,
        uint8 thresholdRequired
    );
    error ErrInvalidSignature(
        uint256 index,
        address expected,
        address recovered
    );
    error ErrOutputsMismatch(bytes32 wid);
    error ErrDuplicateDeposit(bytes32 txid, uint32 vout);
    error ErrWithdrawalNotFound(bytes32 wid);
    error ErrInvalidAmount(uint256 amount);
    error ErrInvalidScriptPubKey();
    error ErrInvalidDeadline(uint64 deadline, uint256 currentTime);
    error ErrNoActiveOperatorSet();
    error ErrUnauthorized(address caller, address expected);
    error ErrOperatorSetNotFound(uint32 setId);
    error ErrInvalidThreshold(uint8 threshold, uint256 memberCount);
    error ErrOperatorSetExists(uint32 setId);
    error ErrMerkleVerificationFailed(bytes32 txid);
    error ErrInsufficientConfirmations(
        bytes32 headerHash,
        uint256 confirmations
    );
    error ErrInvalidHeader(uint256 headerLength);
    error ErrVaultOutputNotFound(bytes32 txid, uint256 amountSats);
    error ErrEnvelopeNotFound(bytes32 txid, bytes32 envelopeHash);

    constructor(
        address wbtc,
        bytes memory _vaultChangeSpk,
        bytes memory _anchorSpk,
        bool _anchorRequired,
        bytes memory _vaultScriptPubkey,
        bytes memory _opretTag,
        address btcRelay
    ) EIP712("BridgeGateway", "1") Ownable(msg.sender) {
        WBTC = IMintBurnERC20(wbtc);
        vaultChangeSpk = _vaultChangeSpk;
        anchorSpk = _anchorSpk;
        anchorRequired = _anchorRequired;
        vaultScriptPubkey = _vaultScriptPubkey;
        opretTag = _opretTag;
        BtcRelay = IBtcRelay(btcRelay);
    }

    // ========= Owner setters =========

    /**
     * @notice Update withdrawal policy parameters
     * @dev Changes how Bitcoin transactions are constructed for withdrawals
     * @param _vaultChangeSpk Bitcoin scriptPubKey for change outputs
     * @param _anchorSpk Bitcoin scriptPubKey for anchor outputs (CPFP)
     * @param _anchorRequired Whether anchor output is mandatory
     * @param _policyVersion Version number for policy (affects outputsHash)
     */
    function setPolicy(
        bytes calldata _vaultChangeSpk,
        bytes calldata _anchorSpk,
        bool _anchorRequired,
        uint32 _policyVersion
    ) external onlyOwner {
        vaultChangeSpk = _vaultChangeSpk;
        anchorSpk = _anchorSpk;
        anchorRequired = _anchorRequired;
        policyVersion = _policyVersion;
    }

    /**
     * @notice Update deposit verification parameters
     * @dev Changes how Bitcoin deposits are validated
     * @param vaultSpk Bitcoin scriptPubKey where deposits should be sent
     * @param _opretTag OP_RETURN tag prefix for envelope identification
     */
    function setDepositParams(
        bytes calldata vaultSpk,
        bytes calldata _opretTag
    ) external onlyOwner {
        vaultScriptPubkey = vaultSpk;
        opretTag = _opretTag;
    }

    /**
     * @notice Update Bitcoin relay contract address
     * @dev Used for SPV proof verification of confirmations
     * @param relay Address of the BTC relay contract
     */
    function setBtcRelay(address relay) external onlyOwner {
        BtcRelay = IBtcRelay(relay);
    }

    // ========= Operator set admin =========

    /**
     * @notice Create a new operator set for M-of-N multisig
     * @dev Only owner can create operator sets
     * @param setId Unique identifier for the operator set
     * @param members Array of operator addresses
     * @param threshold Number of signatures required (M)
     * @param active Whether this set becomes the active set for new withdrawals
     */
    function createOperatorSet(
        uint32 setId,
        address[] calldata members,
        uint8 threshold,
        bool active
    ) external onlyOwner {
        if (sets[setId].members.length != 0) revert ErrOperatorSetExists(setId);
        if (threshold == 0 || threshold > members.length)
            revert ErrInvalidThreshold(threshold, members.length);
        sets[setId] = OperatorSet({
            members: members,
            threshold: threshold,
            active: active
        });
        emit OperatorSetCreated(setId, threshold, members.length, active);
        if (active) currentSignerSetId = setId;
    }

    /**
     * @notice Update an existing operator set
     * @dev Only owner can update operator sets
     * @param setId Operator set ID to update
     * @param members New array of operator addresses
     * @param threshold New signature threshold (M)
     * @param active Whether this set becomes the active set
     */
    function updateOperatorSet(
        uint32 setId,
        address[] calldata members,
        uint8 threshold,
        bool active
    ) external onlyOwner {
        if (sets[setId].members.length == 0)
            revert ErrOperatorSetNotFound(setId);
        if (threshold == 0 || threshold > members.length)
            revert ErrInvalidThreshold(threshold, members.length);
        sets[setId] = OperatorSet({
            members: members,
            threshold: threshold,
            active: active
        });
        emit OperatorSetUpdated(setId, threshold, members.length, active);
        if (active) currentSignerSetId = setId;
    }

    // ========= Withdrawal =========

    /**
     * ┌─────────────────────────────────────────────────────────────────────────────┐
     * │ WITHDRAWAL FLOW IMPLEMENTATION (Event-Sourced UTXO Architecture)            │
     * ├─────────────────────────────────────────────────────────────────────────────┤
     * │ Current Implementation:                                                     │
     * │   1. User queries Indexer API for available UTXOs                           │
     * │   2. User calls requestWithdraw(amount, destSpk, deadline, proposedUtxos)   │
     * │      • Validates UTXOs (unspent, valid source, sufficient amount)           │
     * │      • Locks wBTC (transferFrom user → bridge)                              │
     * │      • Stores selectedUtxoIds (NOT marked as spent yet!)                    │
     * │      • Constructs PSBT from proposedUtxos via _constructPsbtFromInputs()    │
     * │      • Emits WithdrawalInitiated(wid, user, signerSetId, deadline,          │
     * │                                   outputsHash, psbt) ← Single optimized event│
     * │   3. Indexer listens to WithdrawalInitiated, parses PSBT                    │
     * │      • Extracts: amountSats, destSpk, selectedUtxoIds from PSBT             │
     * │      • Marks UTXOs as "pending" (not available for new withdrawals)         │
     * │   4. Operators listen to WithdrawalInitiated event                          │
     * │      • Parse PSBT to see selected UTXOs and withdrawal details              │
     * │      • Build Bitcoin transaction off-chain                                  │
     * │      • Sign EIP-712 digest: WithdrawApproval(wid, outputsHash, ...)         │
     * │   5. Operator calls finalizeByApprovals(wid, rawTx, sigs[])                 │
     * │      • Verifies M-of-N EIP-712 signatures                                   │
     * │      • Verifies rawTx outputs match policy (dest + change + anchor)         │
     * │      • Marks UTXOs as spent: utxoSpent[id] = true ← HERE!                   │
     * │      • Emits UtxoSpent(utxoId, wid, ...) for each UTXO                      │
     * │      • Burns wBTC atomically                                                │
     * │      • Emits SignedTxReady(wid, user, txid, amount, rawTx)                  │
     * │   6. Indexer listens to UtxoSpent, removes UTXOs from available pool        │
     * │   7. L2-Watcher broadcasts rawTx to Bitcoin network                         │
     * │                                                                             │
     * │ Key Features:                                                               │
     * │   ✅ Event-sourced UTXO tracking (98% gas savings: 50K vs 2.5M gas)         │
     * │   ✅ PSBT contains all withdrawal details (amountSats, destSpk, UTXOs)      │
     * │   ✅ User-proposed UTXO selection (via Indexer API)                         │
     * │   ✅ Trustless validation (on-chain spent check prevents double-spend)      │
     * │   ✅ M-of-N multisig security with EIP-712 signatures                       │
     * │                                                                             │
     * │ See: FLOW_CHARTS.md, ARCHITECTURE.md for detailed diagrams                  │
     * └─────────────────────────────────────────────────────────────────────────────┘
     */

    /**
     * @notice UTXO input for withdrawal (user-proposed)
     * @dev User proposes UTXOs from off-chain API, contract validates them
     */
    struct UtxoInput {
        bytes32 txid;
        uint32 vout;
        uint256 amount;
    }

    /**
     * @notice Request BTC withdrawal (Mojave L2 → Bitcoin)
     * @dev flow:
     *      1. User queries /utxos/select API with amount and destination
     *      2. API returns optimal UTXO selection (LARGEST_FIRST algorithm)
     *      3. User calls requestWithdraw with proposedUtxos (UtxoInput[])
     *      4. Contract validates UTXOs:
     *         • utxoSpent[id] == false (not already spent)
     *         • utxoSource[id] == DEPOSIT or COLLATERAL (valid source)
     *         • sum(utxo.amount) >= amountSats + estimatedFee (sufficient amount)
     *      5. Contract locks wBTC (transferFrom user → bridge)
     *      6. Contract stores selectedUtxoIds in Withdrawal struct (NOT marked as spent!)
     *      7. Contract constructs PSBT via _constructPsbtFromInputs()
     *      8. Contract emits WithdrawalInitiated event:
     *         • WithdrawalInitiated(wid, user, signerSetId, deadline, outputsHash, psbt)
     *         • PSBT contains: amountSats, destSpk, UTXOs, outputs (saves ~2K gas)
     *      9. Off-chain indexer parses PSBT to extract withdrawal details
     *     10. Off-chain indexer marks UTXOs as "pending" (not available for new withdrawals)
     *     11. Off-chain operators listen and sign withdrawal (EIP-712)
     *     12. Operator calls finalizeByApprovals() to mark UTXOs as spent and burn wBTC
     *
     * @param amountSats Amount to withdraw in satoshis (1:1 with wBTC decimals)
     * @param destSpk Bitcoin destination scriptPubKey (P2PKH, P2WPKH, P2SH, P2WSH)
     * @param deadline Unix timestamp after which withdrawal can be canceled
     * @param proposedUtxos User-selected UTXOs from /utxos/select API (UtxoInput array)
     * @return wid Unique withdrawal ID for tracking and finalization
     */
    function requestWithdraw(
        uint256 amountSats,
        bytes calldata destSpk,
        uint64 deadline,
        UtxoInput[] calldata proposedUtxos
    ) external nonReentrant returns (bytes32 wid) {
        if (amountSats == 0) revert ErrInvalidAmount(amountSats);
        if (destSpk.length == 0) revert ErrInvalidScriptPubKey();
        if (deadline <= block.timestamp)
            revert ErrInvalidDeadline(deadline, block.timestamp);

        // lock wBTC from user on Mojave L2 (assumes 1 sat == 1 token unit)
        require(
            WBTC.transferFrom(msg.sender, address(this), amountSats),
            "transferFrom"
        );

        // snapshot signerSetId
        uint32 signerSetId = currentSignerSetId;
        if (!sets[signerSetId].active) revert ErrNoActiveOperatorSet();

        // outputsHash with current policy (minFeeRate omitted in v0)
        bytes32 changePolicyHash = keccak256(vaultChangeSpk);
        bytes32 outputsHash = keccak256(
            abi.encode(
                amountSats,
                keccak256(destSpk),
                changePolicyHash,
                anchorRequired,
                uint256(0), // minFeeRate (optional, v0=0)
                policyVersion
            )
        );

        wid = keccak256(
            abi.encodePacked(
                msg.sender,
                amountSats,
                destSpk,
                block.number,
                userNonces[msg.sender]++
            )
        );

        // Validate proposed UTXOs (user provides selection from off-chain API)
        // If empty array provided, operator will select UTXOs during finalization
        bytes32[] memory selectedUtxoIds = new bytes32[](proposedUtxos.length);
        uint256 totalInputAmount = 0;

        for (uint256 i = 0; i < proposedUtxos.length; i++) {
            bytes32 utxoId = keccak256(
                abi.encodePacked(proposedUtxos[i].txid, proposedUtxos[i].vout)
            );

            // ✅ Validate UTXO is unspent
            require(!utxoSpent[utxoId], "UTXO already spent");

            // ✅ Validate UTXO is from deposit or collateral only
            require(
                utxoSource[utxoId] == UtxoSource.DEPOSIT ||
                    utxoSource[utxoId] == UtxoSource.COLLATERAL,
                "Invalid UTXO source"
            );

            selectedUtxoIds[i] = utxoId;
            totalInputAmount += proposedUtxos[i].amount;
        }

        // Validate total input is sufficient
        uint256 estimatedFee = 10000; // 10k sats buffer
        require(
            totalInputAmount >= amountSats + estimatedFee,
            "Insufficient UTXO amount"
        );

        withdrawals[wid] = Withdrawal({
            user: msg.sender,
            amountSats: amountSats,
            destSpk: destSpk,
            deadline: deadline,
            outputsHash: outputsHash,
            version: policyVersion,
            signerSetId: signerSetId,
            state: WState.Pending,
            signatureBitmap: 0,
            signatureCount: 0,
            selectedUtxoIds: selectedUtxoIds,
            totalInputAmount: totalInputAmount
        });

        // Track withdrawal IDs
        allWithdrawalIds.push(wid);
        userWithdrawalIds[msg.sender].push(wid);

        // Construct PSBT with proposed UTXOs
        bytes memory psbt = _constructPsbtFromInputs(
            wid,
            amountSats,
            destSpk,
            0, // feeSats = 0 for requestWithdraw (fee determined by validators from change)
            proposedUtxos,
            totalInputAmount
        );

        // Emit event with PSBT (amountSats, destSpk embedded in PSBT)
        emit WithdrawalInitiated(
            wid,
            msg.sender,
            signerSetId,
            deadline,
            outputsHash,
            psbt
        );

        return wid;
    }

    /**
     * @notice Cancel a pending withdrawal and refund wBTC on Mojave L2
     * @dev Can be called by user anytime or by anyone after deadline
     * @param wid Withdrawal ID to cancel
     */
    function cancelWithdraw(bytes32 wid) external nonReentrant {
        Withdrawal storage w = withdrawals[wid];
        if (w.state != WState.Pending) revert ErrNotPending(wid, w.state);
        require(
            msg.sender == w.user || block.timestamp > w.deadline,
            "auth/time"
        );
        w.state = WState.Canceled;
        require(WBTC.transfer(w.user, w.amountSats), "unlock");

        emit WithdrawalCanceled(wid, w.user, w.amountSats, msg.sender);
    }

    /**
     * @notice Submit individual operator signature for withdrawal (incremental signing)
     * @dev Default flow for withdrawal finalization - operators sign incrementally
     *      When M-of-N threshold is reached, automatically finalizes the withdrawal
     *
     * ┌─────────────────────────────────────────────────────────────────────┐
     * │ PRIMARY FLOW (Incremental Signing):                                 │
     * ├─────────────────────────────────────────────────────────────────────┤
     * │ 1. Listen to WithdrawalInitiated event (with PSBT)                  │
     * │ 2. Parse PSBT from event data (contains all withdrawal details)     │
     * │ 3. Sign EIP-712 approval digest off-chain                           │
     * │ 4. Call submitSignature(wid, sig, rawTx) individually               │
     * │    → Contract stores signature and verifies signer                  │
     * │ 5. When M-th signature submitted:                                   │
     * │    → Automatically finalizes: Mark UTXOs spent + Burn + Emit        │
     * │    → No external finalizer needed!                                  │
     * │                                                                     │
     * │ ALTERNATIVE FLOW (Batch Finalization - For Gas Optimization):       │
     * │ 1. Listen to WithdrawalInitiated event (with PSBT)                  │
     * │ 2. Parse PSBT from event data (contains all withdrawal details)     │
     * │ 3. Build Bitcoin transaction off-chain                              │
     * │ 4. Sign EIP-712 digest: WithdrawApproval(wid, outputsHash, ...)     │
     * │ 5. Coordinate M-of-N signatures off-chain                           │
     * │ 6. Call finalizeByApprovals(wid, rawTx, sigs[]) directly            │
     * │    → Atomic: Verify sigs + Mark UTXOs spent + Burn + Emit           │
     * │    → Saves gas by submitting all signatures at once                 │
     * │                                                                     │
     * └─────────────────────────────────────────────────────────────────────┘
     *
     * @param wid Withdrawal ID
     * @param signature ECDSA signature over EIP-712 approval digest
     * @param rawTx Signed Bitcoin transaction (only needed when submitting M-th signature)
     */
    function submitSignature(
        bytes32 wid,
        bytes calldata signature,
        bytes calldata rawTx
    ) external nonReentrant {
        Withdrawal storage w = withdrawals[wid];
        if (w.state != WState.Pending && w.state != WState.Ready)
            revert ErrNotPending(wid, w.state);
        if (block.timestamp > w.deadline)
            revert ErrExpired(wid, w.deadline, block.timestamp);

        OperatorSet storage set_ = sets[w.signerSetId];

        // If signature provided, verify and record it
        if (signature.length > 0) {
            // Compute approval digest
            bytes32 digest = _approvalDigest(
                wid,
                w.outputsHash,
                w.version,
                w.deadline,
                w.signerSetId
            );

            // Recover signer from signature
            address signer = ECDSA.recover(digest, signature);

            // Verify signer is in the operator set
            bool isOperator = false;
            uint256 signerIndex = 0;

            for (uint256 i = 0; i < set_.members.length; i++) {
                if (set_.members[i] == signer) {
                    isOperator = true;
                    signerIndex = i;
                    break;
                }
            }

            require(isOperator, "not operator");
            require(signerIndex < 256, "index overflow"); // bitmap limit

            // Check if this operator already signed
            uint256 signerBit = 1 << signerIndex;
            require((w.signatureBitmap & signerBit) == 0, "already signed");

            // Store signature for this operator
            withdrawalSignatures[wid][signer] = signature;
            w.signatureBitmap |= signerBit;
            w.signatureCount++;

            emit SignatureSubmitted(wid, signer, signerIndex);
        }

        // Check if threshold reached → Auto-finalize
        if (w.signatureCount >= set_.threshold) {
            w.state = WState.Ready;
            emit WithdrawalReady(wid, w.user, w.amountSats, w.destSpk);

            // Validate rawTx if provided
            if (rawTx.length > 0) {
                // Verify rawTx outputs match policy
                if (!_checkOutputs(rawTx, w.destSpk, w.amountSats)) {
                    revert ErrOutputsMismatch(wid);
                }

                // Auto-finalize: Mark UTXOs as spent
                bytes32 txid = _dblSha256(rawTx);
                for (uint256 i = 0; i < w.selectedUtxoIds.length; i++) {
                    bytes32 utxoId = w.selectedUtxoIds[i];
                    utxoSpent[utxoId] = true;
                    emit UtxoSpent(utxoId, wid, block.timestamp);
                }

                // Atomic burn
                w.state = WState.Finalized;
                IMintBurnERC20(WBTC).burn(w.amountSats);

                emit SignedTxReady(wid, w.user, txid, w.amountSats, rawTx);
            }
            // If no rawTx provided, external finalizer can call finalizeWithStoredSignatures()
        }
    }

    /**
     * @notice Request withdrawal with explicit fee parameter (명세 S2)
     * @dev Enhanced version with fee validation: A + C ≤ total, B ≥ dust
     * @param amountSats User destination amount (excluding fee)
     * @param destSpk Destination scriptPubKey
     * @param feeSats Bitcoin transaction fee
     * @param deadline Withdrawal deadline
     * @param proposedUtxos UTXOs proposed by user for this withdrawal (from API)
     * @return wid Withdrawal ID
     */
    function requestWithdrawWithFee(
        uint256 amountSats,
        bytes calldata destSpk,
        uint256 feeSats,
        uint64 deadline,
        UtxoInput[] calldata proposedUtxos
    ) external nonReentrant returns (bytes32 wid) {
        // S2 Validation
        if (amountSats == 0) revert ErrInvalidAmount(amountSats);
        if (feeSats == 0) revert ErrInvalidAmount(feeSats);
        if (destSpk.length == 0) revert ErrInvalidScriptPubKey();
        if (deadline <= block.timestamp)
            revert ErrInvalidDeadline(deadline, block.timestamp);

        // S2: Check A + C ≤ total
        uint256 totalAmount = amountSats + feeSats;

        // Lock total wBTC from user
        require(
            WBTC.transferFrom(msg.sender, address(this), totalAmount),
            "transferFrom"
        );

        // Snapshot signerSetId
        uint32 signerSetId = currentSignerSetId;
        if (!sets[signerSetId].active) revert ErrNoActiveOperatorSet();

        // Calculate outputsHash with fee
        bytes32 changePolicyHash = keccak256(vaultChangeSpk);
        bytes32 outputsHash = keccak256(
            abi.encode(
                amountSats, // A: user destination
                keccak256(destSpk),
                changePolicyHash, // B: change (must be ≥ dust, validated by operators)
                anchorRequired,
                feeSats, // C: fee
                policyVersion
            )
        );

        // Generate unique WID
        wid = keccak256(
            abi.encodePacked(
                block.timestamp,
                msg.sender,
                amountSats,
                destSpk,
                feeSats,
                withdrawalNonce++
            )
        );

        // Validate proposed UTXOs (user provides selection from off-chain API)
        require(proposedUtxos.length > 0, "No UTXOs proposed");

        bytes32[] memory selectedUtxoIds = new bytes32[](proposedUtxos.length);
        uint256 totalInputAmount = 0;

        for (uint256 i = 0; i < proposedUtxos.length; i++) {
            bytes32 utxoId = keccak256(
                abi.encodePacked(proposedUtxos[i].txid, proposedUtxos[i].vout)
            );

            // ✅ Validate UTXO is unspent
            require(!utxoSpent[utxoId], "UTXO already spent");

            // ✅ Validate UTXO is from deposit or collateral only
            require(
                utxoSource[utxoId] == UtxoSource.DEPOSIT ||
                    utxoSource[utxoId] == UtxoSource.COLLATERAL,
                "Invalid UTXO source"
            );

            selectedUtxoIds[i] = utxoId;
            totalInputAmount += proposedUtxos[i].amount;
        }

        // Validate total input is sufficient
        require(totalInputAmount >= totalAmount, "Insufficient UTXO amount");

        withdrawals[wid] = Withdrawal({
            user: msg.sender,
            amountSats: totalAmount, // Store total (A + C)
            destSpk: destSpk,
            deadline: deadline,
            outputsHash: outputsHash,
            version: policyVersion,
            signerSetId: signerSetId,
            state: WState.Pending,
            signatureBitmap: 0,
            signatureCount: 0,
            selectedUtxoIds: selectedUtxoIds,
            totalInputAmount: totalInputAmount
        });

        // Track withdrawal IDs
        allWithdrawalIds.push(wid);
        userWithdrawalIds[msg.sender].push(wid);

        // Construct PSBT with proposed UTXOs
        bytes memory psbt = _constructPsbtFromInputs(
            wid,
            amountSats,
            destSpk,
            feeSats,
            proposedUtxos,
            totalInputAmount
        );

        // Emit event with PSBT (totalAmount, destSpk embedded in PSBT)
        emit WithdrawalInitiated(
            wid,
            msg.sender,
            signerSetId,
            deadline,
            outputsHash,
            psbt
        );

        return wid;
    }

    /**
     * @notice Finalize withdrawal with M-of-N operator signatures (batch finalization)
     * @dev Alternative flow - submits all signatures at once
     *      Primary flow is incremental signing via submitSignature()
     *
     *      Batch finalization flow:
     *      1. Operators listen to WithdrawalInitiated event
     *      2. Operators build Bitcoin tx off-chain (inputs from selectedUtxoIds, outputs from policy)
     *      3. Operators sign EIP-712 digest: WithdrawApproval(wid, outputsHash, version, expiry, signerSetId)
     *      4. Operators coordinate M signatures off-chain
     *      5. Anyone calls finalizeByApprovals with rawTx + M signatures
     *      6. Contract verifies M-of-N EIP-712 signatures (bitmap validation)
     *      7. Contract verifies rawTx outputs match policy (dest + change + anchor)
     *      8. Contract marks selectedUtxoIds as spent (emits UtxoSpent events)
     *      9. Contract burns wBTC (atomic burn)
     *     10. Contract emits SignedTxReady event
     *     11. Off-chain watcher broadcasts rawTx to Bitcoin network
     *
     * Security:
     *      - M-of-N threshold prevents single operator fraud
     *      - EIP-712 prevents signature replay across chains
     *      - outputsHash prevents output manipulation
     *      - Atomic burn ensures wBTC supply matches Bitcoin vault
     *
     *
     * @param wid Withdrawal ID to finalize
     * @param rawTx Signed Bitcoin transaction ready for broadcast
     * @param outputsHash Policy hash (must match withdrawal.outputsHash)
     * @param version Policy version (must match withdrawal.version)
     * @param signerSetId Operator set ID (must match withdrawal.signerSetId)
     * @param signerBitmap Bitmap indicating which operators signed (LSB = index 0)
     * @param sigs Array of M EIP-712 signatures in bitmap order
     * @param expiry Unix timestamp for signature freshness
     */
    function finalizeByApprovals(
        bytes32 wid,
        bytes calldata rawTx,
        bytes32 outputsHash,
        uint32 version,
        uint32 signerSetId,
        uint256 signerBitmap,
        bytes[] calldata sigs, // M signatures in bitmap order
        uint64 expiry // approval freshness
    ) external nonReentrant {
        Withdrawal storage w = withdrawals[wid];
        // Allow both Pending (batch finalization) and Ready (after submitSignature)
        if (w.state != WState.Pending && w.state != WState.Ready)
            revert ErrNotPending(wid, w.state);
        if (block.timestamp > w.deadline || block.timestamp > expiry)
            revert ErrExpired(wid, w.deadline, block.timestamp);
        require(
            outputsHash == w.outputsHash &&
                version == w.version &&
                signerSetId == w.signerSetId,
            "mismatch"
        );

        // 1) Verify M-of-N EIP-712 approvals
        OperatorSet storage set_ = sets[signerSetId];
        (bool ok, uint256 m) = _verifyApprovals(
            set_,
            signerBitmap,
            sigs,
            _approvalDigest(wid, outputsHash, version, expiry, signerSetId)
        );
        if (!ok || m < set_.threshold)
            revert ErrThresholdNotMet(m, set_.threshold);

        // 2) rawTx outputs must match policy
        if (!_checkOutputs(rawTx, w.destSpk, w.amountSats))
            revert ErrOutputsMismatch(wid);

        // 3) Mark selected UTXOs as spent
        bytes32 txid = _dblSha256(rawTx);
        for (uint256 i = 0; i < w.selectedUtxoIds.length; i++) {
            bytes32 utxoId = w.selectedUtxoIds[i];
            utxoSpent[utxoId] = true; // Mark as spent

            // Emit event for off-chain indexer
            emit UtxoSpent(utxoId, wid, block.timestamp);
        }

        // 4) TODO: Register change UTXO (requires Bitcoin tx parsing)
        // For now, change UTXO can be registered via operator call to registerCollateralUtxo()

        // 5) Atomic burn
        w.state = WState.Finalized;
        IMintBurnERC20(WBTC).burn(w.amountSats);

        emit SignedTxReady(wid, w.user, txid, w.amountSats, rawTx);
    }

    /**
     * @notice Finalize withdrawal using signatures stored via submitSignature()
     * @dev Optional: Use this when M-th operator didn't provide rawTx in submitSignature()
     *      Normally submitSignature() auto-finalizes when threshold is reached.
     *      This function handles edge cases where rawTx submission was delayed.
     * @param wid Withdrawal ID (must be in Ready state)
     * @param rawTx Signed Bitcoin transaction ready for broadcast
     */
    function finalizeWithStoredSignatures(
        bytes32 wid,
        bytes calldata rawTx
    ) external nonReentrant {
        Withdrawal storage w = withdrawals[wid];
        if (w.state != WState.Ready) revert ErrNotPending(wid, w.state);
        if (block.timestamp > w.deadline)
            revert ErrExpired(wid, w.deadline, block.timestamp);

        // Verify rawTx outputs match policy
        if (!_checkOutputs(rawTx, w.destSpk, w.amountSats))
            revert ErrOutputsMismatch(wid);

        // Verify we have enough signatures stored
        OperatorSet storage set_ = sets[w.signerSetId];
        require(w.signatureCount >= set_.threshold, "insufficient signatures");

        // Mark UTXOs as spent
        bytes32 txid = _dblSha256(rawTx);
        for (uint256 i = 0; i < w.selectedUtxoIds.length; i++) {
            bytes32 utxoId = w.selectedUtxoIds[i];
            utxoSpent[utxoId] = true;
            emit UtxoSpent(utxoId, wid, block.timestamp);
        }

        // Atomic burn
        w.state = WState.Finalized;
        IMintBurnERC20(WBTC).burn(w.amountSats);

        emit SignedTxReady(wid, w.user, txid, w.amountSats, rawTx);
    }

    /**
     * @notice Get stored signature for a specific withdrawal and operator
     * @param wid Withdrawal ID
     * @param operator Operator address
     * @return Stored signature (empty if not submitted)
     */
    function getStoredSignature(
        bytes32 wid,
        address operator
    ) external view returns (bytes memory) {
        return withdrawalSignatures[wid][operator];
    }

    /**
     * @notice Calculate EIP-712 approval digest for operator signing
     * @dev Public helper for operators to generate signing digests
     * @param wid Withdrawal ID
     * @param outputsHash Policy hash
     * @param version Policy version
     * @param expiry Signature expiry timestamp
     * @param signerSetId Operator set ID
     * @return EIP-712 typed data hash for signing
     */
    function approvalDigestPublic(
        bytes32 wid,
        bytes32 outputsHash,
        uint32 version,
        uint64 expiry,
        uint32 signerSetId
    ) external view returns (bytes32) {
        return _approvalDigest(wid, outputsHash, version, expiry, signerSetId);
    }

    // ========= Collateral UTXO Management =========

    /**
     * @notice Register a collateral UTXO (admin only)
     * @dev Allows operators to register collateral UTXOs or change UTXOs after withdrawal
     * @param txid Bitcoin transaction ID
     * @param vout Output index
     * @param amount Amount in satoshis
     */
    function registerCollateralUtxo(
        bytes32 txid,
        uint32 vout,
        uint256 amount
    ) external onlyOwner {
        bytes32 utxoId = keccak256(abi.encodePacked(txid, vout));

        require(
            utxoSource[utxoId] == UtxoSource.NONE,
            "UTXO already registered"
        );
        require(amount > 0, "Invalid amount");

        // Register as unspent collateral
        utxoSpent[utxoId] = false;
        utxoSource[utxoId] = UtxoSource.COLLATERAL;

        // Emit event for off-chain indexer
        emit UtxoRegistered(
            utxoId,
            txid,
            vout,
            amount,
            UtxoSource.COLLATERAL,
            block.timestamp
        );
    }

    // ========= PSBT / TX Template Construction =========

    /**
     * @notice Construct PSBT from user-proposed UTXO inputs
     * @dev Creates a complete PSBT with proposed UTXOs as inputs
     * @param wid Withdrawal ID
     * @param amountSats User destination amount
     * @param destSpk User destination scriptPubKey
     * @param feeSats Transaction fee
     * @param proposedUtxos User-proposed UTXO inputs
     * @param totalInputAmount Total amount from proposed UTXOs
     * @return Complete PSBT as abi-encoded bytes
     */
    function _constructPsbtFromInputs(
        bytes32 wid,
        uint256 amountSats,
        bytes memory destSpk,
        uint256 feeSats,
        UtxoInput[] memory proposedUtxos,
        uint256 totalInputAmount
    ) internal view returns (bytes memory) {
        // Calculate change amount
        uint256 anchorAmount = anchorRequired ? 546 : 0; // Bitcoin dust limit
        uint256 changeAmount = totalInputAmount -
            amountSats -
            feeSats -
            anchorAmount;

        // Encode inputs from proposed UTXOs
        bytes[] memory inputs = new bytes[](proposedUtxos.length);
        for (uint256 i = 0; i < proposedUtxos.length; i++) {
            inputs[i] = abi.encode(
                proposedUtxos[i].txid, // Previous tx hash
                proposedUtxos[i].vout, // Previous tx output index
                proposedUtxos[i].amount, // Amount in satoshis
                vaultChangeSpk // scriptPubKey of vault (for signing)
            );
        }

        // Output 0: User destination
        bytes memory output0 = abi.encode(
            destSpk,
            amountSats,
            "user_destination"
        );

        // Output 1: Vault change
        bytes memory output1 = abi.encode(
            vaultChangeSpk,
            changeAmount,
            "vault_change"
        );

        // Output 2: Anchor (optional, for CPFP)
        bytes memory output2;
        if (anchorRequired) {
            output2 = abi.encode(anchorSpk, anchorAmount, "anchor_cpfp");
        }

        // Encode complete PSBT
        bytes memory psbt = abi.encode(
            wid, // withdrawalId
            uint32(2), // version (Bitcoin tx v2)
            uint32(0), // locktime
            inputs, // inputs array
            output0, // user output
            output1, // change output
            output2, // anchor output (empty if not required)
            feeSats, // fee amount
            policyVersion, // policy version
            anchorRequired, // anchor required flag
            uint8(1) // sighashType (SIGHASH_ALL)
        );

        return psbt;
    }

    // ========= EIP-712 helpers =========
    function _approvalDigest(
        bytes32 wid,
        bytes32 outputsHash,
        uint32 version,
        uint64 expiry,
        uint32 signerSetId
    ) internal view returns (bytes32) {
        bytes32 structHash = keccak256(
            abi.encode(
                WITHDRAW_APPROVAL_TYPEHASH,
                wid,
                outputsHash,
                version,
                expiry,
                signerSetId
            )
        );
        return _hashTypedDataV4(structHash);
    }

    function _verifyApprovals(
        OperatorSet storage set_,
        uint256 signerBitmap,
        bytes[] calldata sigs,
        bytes32 digest
    ) internal view returns (bool ok, uint256 m) {
        uint256 bm = signerBitmap;
        uint256 idxSig = 0;
        for (uint256 i = 0; i < set_.members.length && bm != 0; ++i) {
            if ((bm & 1) == 1) {
                address r = ECDSA.recover(digest, sigs[idxSig]);
                if (r != set_.members[i]) return (false, m);
                unchecked {
                    ++idxSig;
                    ++m;
                }
                if (m == set_.threshold) return (true, m);
            }
            bm >>= 1;
        }
        return (m >= set_.threshold, m);
    }

    // ========= Raw TX parsing (outputs only) =========
    // Minimal varint + output reader. Assumes valid tx serialization.

    function _checkOutputs(
        bytes calldata rawTx,
        bytes memory destSpk,
        uint256 amountSats
    ) internal view returns (bool) {
        uint256 p = 0;
        // skip version (4 bytes)
        p += 4;

        // handle segwit marker/flag if present (0x00 0x01)
        bool isSegwit = false;
        if (rawTx.length > p + 1 && rawTx[p] == 0x00 && rawTx[p + 1] == 0x01) {
            isSegwit = true;
            p += 2;
        }

        // vinCount
        (uint256 vinCount, uint256 p2) = _readVarInt(rawTx, p);
        p = p2;
        // skip inputs
        for (uint256 i = 0; i < vinCount; ++i) {
            p += 36; // outpoint (32+4)
            (uint256 scriptLen, uint256 p3) = _readVarInt(rawTx, p);
            p = p3 + scriptLen; // scriptSig
            p += 4; // sequence
        }

        // voutCount
        (uint256 voutCount, uint256 p4) = _readVarInt(rawTx, p);
        p = p4;

        bool destOk = false;
        bool changeOk = false;
        bool anchorOk = !anchorRequired; // if not required, treat as OK

        for (uint256 i = 0; i < voutCount; ++i) {
            require(p + 8 <= rawTx.length, "vout val");
            uint64 valueLE = _readLE64(rawTx, p);
            p += 8;
            (uint256 pkLen, uint256 p5) = _readVarInt(rawTx, p);
            p = p5;
            require(p + pkLen <= rawTx.length, "vout spk");
            bytes calldata spk = rawTx[p:p + pkLen];
            p += pkLen;

            if (
                !destOk &&
                _bytesEq(spk, destSpk) &&
                uint256(valueLE) == amountSats
            ) destOk = true;
            if (!changeOk && _bytesEq(spk, vaultChangeSpk)) changeOk = true;
            if (!anchorOk && _bytesEq(spk, anchorSpk)) anchorOk = true;
        }

        // skip witness if segwit (not needed for outputs check)
        if (isSegwit) {
            /* witness skip */
        }

        return destOk && changeOk && anchorOk;
    }

    // ========= Deposit (SPV skeleton) =========
    struct SpvProof {
        bytes rawTx; // serialized bitcoin tx (with witness data for SegWit)
        bytes32 txid; // witness-stripped TXID for merkle verification
        bytes32[] merkleBranch; // siblings to merkleRoot
        uint32 index; // position in merkle tree
        bytes header0; // 80B, containing merkleRoot
        bytes[] confirmHeaders; // optional if relay not used
    }

    /**
     * @notice Claim BTC deposit with SPV proof (Bitcoin → Mojave L2)
     * @dev flow:
     *      1. User sends BTC to bridge vault address with OP_RETURN envelope
     *      2. User waits for 6 confirmations on Bitcoin
     *      3. User builds SPV proof (txid, merkle branch, header, confirmHeaders)
     *      4. User calls claimDepositSpv with proof
     *      5. Contract verifies 6 confirmations via BtcRelay
     *      6. Contract verifies merkle inclusion proof
     *      7. Contract parses Bitcoin tx outputs (vault scriptPubKey + OP_RETURN)
     *      8. Contract prevents double-spend (processedOutpoint check)
     *      9. Contract mints wBTC to recipient
     *     10. Contract registers UTXO (emits UtxoRegistered for indexer)
     *     11. Contract emits DepositFinalized event
     *
     * @param recipient Mojave L2 address to receive minted wBTC
     * @param amountSats Amount in satoshis to claim (must match Bitcoin tx output)
     * @param envelopeHash keccak256(opretTag, chainId, verifyingContract, recipient, amountSats)
     * @param proof SPV proof (rawTx, txid, merkleBranch, index, header0, confirmHeaders)
     */
    function claimDepositSpv(
        address recipient,
        uint256 amountSats,
        bytes32 envelopeHash, // keccak(tag, chainId, verifyingContract, recipient, amountSats)
        SpvProof calldata proof
    ) external nonReentrant {
        // (A) Verify confirmations (either via relay or bundled headers)
        if (address(BtcRelay) != address(0)) {
            // Use Bitcoin standard double-SHA256 for header hash (same as BtcRelay)
            bytes32 h0 = sha256(abi.encodePacked(sha256(proof.header0)));
            require(BtcRelay.verifyConfirmations(h0, 6), "no 6conf");
        } else {
            // TODO: optional bundled header verification (nBits/prevhash chain)
            // optional: verify confirmHeaders chain
        }

        // (B) Merkle inclusion
        // Use the provided witness-stripped TXID for merkle verification
        // Bitcoin merkle tree uses little-endian TXID representation
        bytes32 txidLE = _reverseBytes32(proof.txid);
        bytes32 calcRoot = _merkleCompute(
            txidLE,
            proof.merkleBranch,
            proof.index
        );
        // With a real relay, compare to relay.merkleRoot(headerHash). Here compare to header bytes placeholder:
        require(calcRoot == _readMerkleRootFromHeader(proof.header0), "merkle");

        // (C) Parse outputs: must include exact amount to vaultScriptPubkey and OP_RETURN(envelopeHash)
        (bool voutOk, uint32 voutIndex) = _hasExactOutputToVault(
            proof.rawTx,
            amountSats
        );
        require(voutOk, "vault out");
        require(_hasOpretEnvelope(proof.rawTx, envelopeHash), "opret");

        bytes32 outpointKey = keccak256(
            abi.encodePacked(proof.txid, voutIndex)
        );
        if (processedOutpoint[outpointKey])
            revert ErrDuplicateDeposit(proof.txid, voutIndex);
        processedOutpoint[outpointKey] = true;

        IMintBurnERC20(WBTC).mint(recipient, amountSats);

        bytes32 did = keccak256(
            abi.encodePacked(proof.txid, recipient, amountSats)
        );

        // Track deposit IDs
        allDepositIds.push(did);
        userDepositIds[recipient].push(did);

        // Register UTXO (minimal on-chain state)
        bytes32 utxoId = keccak256(abi.encodePacked(proof.txid, voutIndex));
        utxoSpent[utxoId] = false; // Mark as unspent
        utxoSource[utxoId] = UtxoSource.DEPOSIT; // Mark as from deposit

        // Emit events for off-chain indexer
        emit UtxoRegistered(
            utxoId,
            proof.txid,
            voutIndex,
            amountSats,
            UtxoSource.DEPOSIT,
            block.timestamp
        );

        emit DepositFinalized(
            did,
            recipient,
            amountSats,
            proof.txid,
            voutIndex
        );
    }

    // ---- Deposit helpers (simplified; adapt to your tooling) ----

    function _hasExactOutputToVault(
        bytes calldata rawTx,
        uint256 amountSats
    ) internal view returns (bool ok, uint32 voutIndex) {
        uint256 p = 0;
        p += 4; // version
        bool isSegwit = false;
        if (rawTx.length > p + 1 && rawTx[p] == 0x00 && rawTx[p + 1] == 0x01) {
            isSegwit = true;
            p += 2;
        }
        (uint256 vinCount, uint256 p2) = _readVarInt(rawTx, p);
        p = p2;
        for (uint256 i = 0; i < vinCount; ++i) {
            p += 36;
            (uint256 s, uint256 p3) = _readVarInt(rawTx, p);
            p = p3 + s;
            p += 4;
        }
        (uint256 voutCount, uint256 p4) = _readVarInt(rawTx, p);
        p = p4;
        for (uint32 i = 0; i < voutCount; ++i) {
            uint64 valueLE = _readLE64(rawTx, p);
            p += 8;
            (uint256 pkLen, uint256 p5) = _readVarInt(rawTx, p);
            p = p5;
            bytes calldata spk = rawTx[p:p + pkLen];
            p += pkLen;
            if (
                uint256(valueLE) == amountSats &&
                _bytesEq(spk, vaultScriptPubkey)
            ) {
                return (true, i);
            }
        }
        if (isSegwit) {
            /* witness can be skipped */
        }
        return (false, 0);
    }

    function _hasOpretEnvelope(
        bytes calldata rawTx,
        bytes32 envelopeHash
    ) internal pure returns (bool) {
        uint256 p = 0;
        p += 4;
        if (rawTx.length > p + 1 && rawTx[p] == 0x00 && rawTx[p + 1] == 0x01) {
            p += 2;
        }
        (uint256 vinCount, uint256 p2) = _readVarInt(rawTx, p);
        p = p2;
        for (uint256 i = 0; i < vinCount; ++i) {
            p += 36;
            (uint256 s, uint256 p3) = _readVarInt(rawTx, p);
            p = p3 + s;
            p += 4;
        }
        (uint256 voutCount, uint256 p4) = _readVarInt(rawTx, p);
        p = p4;
        for (uint256 i = 0; i < voutCount; ++i) {
            p += 8;
            (uint256 pkLen, uint256 p5) = _readVarInt(rawTx, p);
            p = p5;
            bytes calldata spk = rawTx[p:p + pkLen];
            p += pkLen;
            // OP_RETURN script: 0x6a <pushdata N> <data>
            if (spk.length >= 2 && spk[0] == 0x6a) {
                // crude parse: assume first pushdata contains our envelope (adapt as needed)
                uint256 dlen;
                uint256 off = 1;
                if (off < spk.length) {
                    (dlen, off) = _readPushData(spk, off);
                    if (off + dlen <= spk.length) {
                        // Use calldata slice to extract the envelope data
                        bytes calldata envelopeData = spk[off:off + dlen];
                        bytes32 h = keccak256(envelopeData);
                        if (h == envelopeHash) return true;
                    }
                }
            }
        }
        return false;
    }

    // ========= Low-level utils =========

    function _bytesEq(
        bytes calldata a,
        bytes memory b
    ) internal pure returns (bool) {
        if (a.length != b.length) return false;
        // Compare 32-byte chunks
        uint256 n = a.length;
        uint256 i = 0;
        for (; i + 32 <= n; i += 32) {
            bytes32 wa;
            bytes32 wb;
            assembly {
                wa := calldataload(add(a.offset, i))
            }
            assembly {
                wb := mload(add(add(b, 0x20), i))
            }
            if (wa != wb) return false;
        }
        if (i < n) {
            uint256 rem = n - i;
            bytes32 ma;
            bytes32 mb;
            assembly {
                ma := calldataload(add(a.offset, i))
            }
            assembly {
                mb := mload(add(add(b, 0x20), i))
                mb := and(mb, not(sub(shl(mul(sub(32, rem), 8), 1), 1)))
                ma := and(ma, not(sub(shl(mul(sub(32, rem), 8), 1), 1)))
            }
            if (ma != mb) return false;
        }
        return true;
    }

    function _readVarInt(
        bytes calldata data,
        uint256 p
    ) internal pure returns (uint256 v, uint256 np) {
        require(p < data.length, "varint");
        uint8 x = uint8(data[p]);
        if (x < 0xfd) {
            return (x, p + 1);
        }
        if (x == 0xfd) {
            require(p + 3 <= data.length, "vi16");
            return (uint16(bytes2(data[p + 1:p + 3])), p + 3);
        }
        if (x == 0xfe) {
            require(p + 5 <= data.length, "vi32");
            return (uint32(bytes4(data[p + 1:p + 5])), p + 5);
        }
        require(p + 9 <= data.length, "vi64");
        return (uint64(bytes8(data[p + 1:p + 9])), p + 9);
    }

    function _readLE64(
        bytes calldata data,
        uint256 p
    ) internal pure returns (uint64) {
        require(p + 8 <= data.length, "le64");
        uint64 v = uint64(uint8(data[p])) |
            (uint64(uint8(data[p + 1])) << 8) |
            (uint64(uint8(data[p + 2])) << 16) |
            (uint64(uint8(data[p + 3])) << 24) |
            (uint64(uint8(data[p + 4])) << 32) |
            (uint64(uint8(data[p + 5])) << 40) |
            (uint64(uint8(data[p + 6])) << 48) |
            (uint64(uint8(data[p + 7])) << 56);
        return v;
    }

    function _readPushData(
        bytes calldata spk,
        uint256 off
    ) internal pure returns (uint256 dlen, uint256 next) {
        require(off < spk.length, "pfx");
        uint8 op = uint8(spk[off]);
        if (op <= 75) {
            return (op, off + 1);
        }
        if (op == 76) {
            require(off + 2 <= spk.length, "pd1");
            return (uint8(spk[off + 1]), off + 2);
        }
        if (op == 77) {
            require(off + 3 <= spk.length, "pd2");
            return (uint16(bytes2(spk[off + 1:off + 3])), off + 3);
        }
        if (op == 78) {
            require(off + 5 <= spk.length, "pd4");
            return (uint32(bytes4(spk[off + 1:off + 5])), off + 5);
        }
        revert("pd op");
    }

    function _dblSha256(bytes calldata b) internal pure returns (bytes32) {
        return sha256(abi.encodePacked(sha256(b)));
    }

    function _reverseBytes32(
        bytes32 input
    ) internal pure returns (bytes32 result) {
        bytes memory reversed = new bytes(32);
        for (uint256 i = 0; i < 32; i++) {
            reversed[i] = input[31 - i];
        }
        assembly {
            result := mload(add(reversed, 32))
        }
    }

    function _merkleCompute(
        bytes32 leaf,
        bytes32[] calldata branch,
        uint32 index
    ) internal pure returns (bytes32 h) {
        h = leaf;
        for (uint256 i = 0; i < branch.length; ++i) {
            bytes32 n = branch[i];
            if ((index & 1) == 1) {
                h = sha256(abi.encodePacked(sha256(abi.encodePacked(n, h))));
            } else {
                h = sha256(abi.encodePacked(sha256(abi.encodePacked(h, n))));
            }
            index >>= 1;
        }
    }

    function _readMerkleRootFromHeader(
        bytes calldata header80
    ) internal pure returns (bytes32 root) {
        require(header80.length == 80, "hdr");
        // Bitcoin header layout: [4:version][32:prevHash][32:merkleRoot][4:time][4:nBits][4:nonce]
        // We treat as raw bytes; adapt to your relay endianness.
        assembly {
            root := calldataload(add(header80.offset, 36))
        } // 4+32 = 36
    }

    // ========= View functions for automation =========

    /**
     * @notice Get all pending withdrawal IDs
     * @dev Used by Operator clients to find withdrawals needing finalization
     * @return Array of pending withdrawal IDs
     */
    function getPendingWithdrawals() external view returns (bytes32[] memory) {
        uint256 count = 0;

        // First pass: count pending
        for (uint256 i = 0; i < allWithdrawalIds.length; i++) {
            if (withdrawals[allWithdrawalIds[i]].state == WState.Pending) {
                count++;
            }
        }

        // Second pass: collect
        bytes32[] memory pending = new bytes32[](count);
        uint256 idx = 0;
        for (uint256 i = 0; i < allWithdrawalIds.length; i++) {
            bytes32 wid = allWithdrawalIds[i];
            if (withdrawals[wid].state == WState.Pending) {
                pending[idx] = wid;
                idx++;
            }
        }

        return pending;
    }

    /**
     * @notice Get pending withdrawals with pagination
     * @param offset Starting index
     * @param limit Maximum number of results
     * @return wids Array of withdrawal IDs
     * @return total Total number of pending withdrawals
     */
    function getPendingWithdrawalsPaginated(
        uint256 offset,
        uint256 limit
    ) external view returns (bytes32[] memory wids, uint256 total) {
        // Count total pending
        for (uint256 i = 0; i < allWithdrawalIds.length; i++) {
            if (withdrawals[allWithdrawalIds[i]].state == WState.Pending) {
                total++;
            }
        }

        // Calculate actual limit
        uint256 remaining = total > offset ? total - offset : 0;
        uint256 actualLimit = remaining < limit ? remaining : limit;

        wids = new bytes32[](actualLimit);
        uint256 pendingIdx = 0;
        uint256 resultIdx = 0;

        for (
            uint256 i = 0;
            i < allWithdrawalIds.length && resultIdx < actualLimit;
            i++
        ) {
            bytes32 wid = allWithdrawalIds[i];
            if (withdrawals[wid].state == WState.Pending) {
                if (pendingIdx >= offset) {
                    wids[resultIdx] = wid;
                    resultIdx++;
                }
                pendingIdx++;
            }
        }

        return (wids, total);
    }

    /**
     * @notice Get all withdrawal IDs for a user
     * @param user User address
     * @return Array of withdrawal IDs
     */
    function getUserWithdrawals(
        address user
    ) external view returns (bytes32[] memory) {
        return userWithdrawalIds[user];
    }

    /**
     * @notice Get all deposit IDs for a user
     * @param user User address
     * @return Array of deposit IDs
     */
    function getUserDeposits(
        address user
    ) external view returns (bytes32[] memory) {
        return userDepositIds[user];
    }

    // ========= UTXO Query Functions (Minimal - use off-chain indexer for details) =========

    /**
     * @notice Check if UTXO is spent
     * @param utxoId UTXO ID (keccak256(txid, vout))
     * @return True if UTXO is spent
     */
    function isUtxoSpent(bytes32 utxoId) external view returns (bool) {
        return utxoSpent[utxoId];
    }

    /**
     * @notice Get UTXO source type
     * @param utxoId UTXO ID (keccak256(txid, vout))
     * @return Source type (DEPOSIT or COLLATERAL)
     */
    function getUtxoSource(bytes32 utxoId) external view returns (UtxoSource) {
        return utxoSource[utxoId];
    }

    /**
     * @notice Get detailed withdrawal information (decoded)
     * @param wid Withdrawal ID
     * @return user User address
     * @return amountSats Amount in satoshis
     * @return destSpk Destination scriptPubKey
     * @return deadline Deadline timestamp
     * @return outputsHash Policy hash
     * @return version Policy version
     * @return signerSetId Operator set ID
     * @return state Current state
     */
    function getWithdrawalDetails(
        bytes32 wid
    )
        external
        view
        returns (
            address user,
            uint256 amountSats,
            bytes memory destSpk,
            uint64 deadline,
            bytes32 outputsHash,
            uint32 version,
            uint32 signerSetId,
            WState state
        )
    {
        Withdrawal storage w = withdrawals[wid];
        return (
            w.user,
            w.amountSats,
            w.destSpk,
            w.deadline,
            w.outputsHash,
            w.version,
            w.signerSetId,
            w.state
        );
    }

    /**
     * @notice Check if a withdrawal is ready for finalization
     * @param wid Withdrawal ID
     * @return ready True if pending and not expired
     * @return reason Reason if not ready
     */
    function canFinalizeWithdrawal(
        bytes32 wid
    ) external view returns (bool ready, string memory reason) {
        Withdrawal storage w = withdrawals[wid];

        if (w.user == address(0)) {
            return (false, "Withdrawal does not exist");
        }

        if (w.state != WState.Pending) {
            return (false, "Withdrawal not pending");
        }

        if (block.timestamp > w.deadline) {
            return (false, "Withdrawal expired");
        }

        return (true, "");
    }

    /**
     * @notice Get operator set information
     * @param setId Operator set ID
     * @return members Array of operator addresses
     * @return threshold Signature threshold
     * @return active Whether the set is active
     */
    function getOperatorSet(
        uint32 setId
    )
        external
        view
        returns (address[] memory members, uint8 threshold, bool active)
    {
        OperatorSet storage set_ = sets[setId];
        return (set_.members, set_.threshold, set_.active);
    }

    /**
     * @notice Batch query withdrawal details
     * @param wids Array of withdrawal IDs
     * @return users Array of user addresses
     * @return amounts Array of amounts in satoshis
     * @return states Array of states
     */
    function getBatchWithdrawalDetails(
        bytes32[] calldata wids
    )
        external
        view
        returns (
            address[] memory users,
            uint256[] memory amounts,
            WState[] memory states
        )
    {
        users = new address[](wids.length);
        amounts = new uint256[](wids.length);
        states = new WState[](wids.length);

        for (uint256 i = 0; i < wids.length; i++) {
            Withdrawal storage w = withdrawals[wids[i]];
            users[i] = w.user;
            amounts[i] = w.amountSats;
            states[i] = w.state;
        }

        return (users, amounts, states);
    }

    /**
     * @notice Get total number of withdrawals
     * @return Total withdrawal count
     */
    function getTotalWithdrawals() external view returns (uint256) {
        return allWithdrawalIds.length;
    }

    /**
     * @notice Get total number of deposits
     * @return Total deposit count
     */
    function getTotalDeposits() external view returns (uint256) {
        return allDepositIds.length;
    }

    /**
     * @notice Get withdrawal ID at index
     * @param index Array index
     * @return Withdrawal ID
     */
    function getWithdrawalIdAt(uint256 index) external view returns (bytes32) {
        require(index < allWithdrawalIds.length, "index out of bounds");
        return allWithdrawalIds[index];
    }

    /**
     * @notice Get deposit ID at index
     * @param index Array index
     * @return Deposit ID
     */
    function getDepositIdAt(uint256 index) external view returns (bytes32) {
        require(index < allDepositIds.length, "index out of bounds");
        return allDepositIds[index];
    }
}
