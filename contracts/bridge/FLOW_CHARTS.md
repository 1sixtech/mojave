# Mojave Bridge Flow Charts

## Complete System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           BITCOIN L1 NETWORK                                │
│                                                                             │
│  ┌──────────────┐                    ┌─────────────────────────────┐        │
│  │              │   BTC + Envelope   │                             │        │
│  │  User Wallet ├───────────────────▶│  Vault Address (Multisig)   │        │
│  │              │                    │                             │        │
│  └──────────────┘                    └─────────────────────────────┘        │
│         │                                        │                          │
│         │                                        │ Physical UTXO Pool       │
│         │                                        ▼                          │
│         │                             ┌──────────────────────┐              │
│         │ OP_RETURN                   │  Bitcoin Blockchain  │              │
│         │ (Envelope)                  │     (Headers)        │              │
│         ▼                             └──────────────────────┘              │
│  ┌──────────────────────┐                        │                          │
│  │ TX with 6+ confirms  │                        │ Header Data              │
│  └──────────────────────┘                        │                          │
└──────────────────────────────────────────────────┼──────────────────────────┘
                                                   │
                            ┌──────────────────────┼──────────────────────┐
                            │                      │                      │
                            │   Bitcoin RPC        │   Header Stream      │
                            │                      │                      │
                            ▼                      ▼                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         MOJAVE L2 (EVM CHAIN)                               │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                      SMART CONTRACTS LAYER                            │  │
│  │                    (Minimal On-Chain State)                           │  │
│  │   ┌──────────────┐      ┌─────────────────┐      ┌──────────────┐     │  │
│  │   │              │      │                 │      │              │     │  │
│  │   │   BtcRelay   │◀─────│ BridgeGateway   │◀────▶│     WBTC     │     │  │
│  │   │              │      │                 │      │  (ERC20)     │     │  │
│  │   │  • Headers   │      │  • Deposits     │      │              │     │  │
│  │   │  • PoW Check │      │  • Withdrawals  │      │  • Mint      │     │  │
│  │   │              │      │  • UTXO Spent   │      │  • Burn      │     │  │
│  │   │              │      │    (bool only)  │      │              │     │  │
│  │   └──────────────┘      └─────────────────┘      └──────────────┘     │  │
│  │                                  │                                    │  │
│  │                                  │ Events:                            │  │
│  │                                  │ • UtxoRegistered                   │  │
│  │                                  │ • UtxoSpent                        │  │
│  └──────────────────────────────────┼────────────────────────────────────┘  │
│                                     │                                       │
│                     ┌───────────────┼───────────────┐                       │
│                     │               │               │                       │
│                     ▼               ▼               ▼                       │
│          ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│          │              │  │              │  │              │               │
│          │  Sequencer   │  │  Operators   │  │  L2-Watcher  │               │
│          │  (Headers)   │  │ (M-of-N Sig) │  │   (Bridge)   │               │
│          │              │  │              │  │              │               │
│          └──────────────┘  └──────────────┘  └──────────────┘               │
│                                     │                 │                     │
│  ┌──────────────────────────────────┼─────────────────┼──────────────────┐  │
│  │         OFF-CHAIN INDEXER API    │                 │                  │  │
│  │         (Event-Sourced State)    │                 │                  │  │
│  │                                  │                 │                  │  │
│  │  Listens to events: ─────────────┘                 │                  │  │
│  │  • UtxoRegistered → Add to pool                    │                  │  │
│  │  • UtxoSpent → Remove from pool                    │                  │  │
│  │                                                    │                  │  │
│  │  Provides APIs:                                    │                  │  │
│  │  • GET /utxos/:address → Available UTXOs           │                  │  │
│  │  • POST /utxos/select → UTXO selection             │                  │  │
│  │  • GET /balance/:address → Total balance           │                  │  │
│  │                                                    │                  │  │
│  │                                                    │                  │  │
│  └──────────────────────────────────┬─────────────────┼──────────────────┘  │
│                                     │                 │                     │
│                                     │ Query UTXOs     │ Broadcast TX        │
│                                     │                 │                     │
└─────────────────────────────────────┼─────────────────┼─────────────────────┘
                                      │                 │
                                      ▼                 ▼
                                  User DApp       Bitcoin L1 Network
```

## Deposit Flow

```
     User                Bitcoin L1           Sequencer          BtcRelay        BridgeGateway      WBTC
      │                      │                    │                 │                  │             │
      │                      │                    │                 │                  │             │
┌─────┴─────────────────────────────────────────────────────────────────────────────────────────────┐
│ 1. User Sends Bitcoin with OP_RETURN                                                              │
└───────────────────────────────────────────────────────────────────────────────────────────────────┘
      │                      │                    │                 │                  │             │
      │  Send BTC + Envelope │                    │                 │                  │             │
      ├─────────────────────▶│                    │                 │                  │             │
      │                      │                    │                 │                  │             │
      │                      │ Mine Block (6+)    │                 │                  │             │
      │                      │───────────┐        │                 │                  │             │
      │                      │           │        │                 │                  │             │
      │                      │◀──────────┘        │                 │                  │             │
      │                      │                    │                 │                  │             │
┌─────┴─────────────────────────────────────────────────────────────────────────────────────────────┐
│ 2. Sequencer Submits Headers (Liveness Component)                                                 │
└───────────────────────────────────────────────────────────────────────────────────────────────────┘
      │                      │                    │                 │                  │             │
      │                      │  Poll New Blocks   │                 │                  │             │
      │                      │◀───────────────────┤                 │                  │             │
      │                      │                    │                 │                  │             │
      │                      │                    │  Submit Header  │                  │             │
      │                      │                    ├────────────────▶│                  │             │
      │                      │                    │                 │                  │             │
      │                      │                    │                 │  Verify PoW      │             │
      │                      │                    │                 │─────────┐        │             │
      │                      │                    │                 │         │        │             │
      │                      │                    │                 │◀────────┘        │             │
      │                      │                    │                 │                  │             │
┌─────┴─────────────────────────────────────────────────────────────────────────────────────────────┐
│ 3. Anyone Claims Deposit with SPV Proof                                                           │
└───────────────────────────────────────────────────────────────────────────────────────────────────┘
      │                      │                    │                 │                  │             │
      │  Get TX + Proof      │                    │                 │                  │             │
      ├─────────────────────▶│                    │                 │                  │             │
      │                      │                    │                 │                  │             │
      │                      │                    │                 │                  │             │
      │  claimDepositSpv()   │                    │                 │                  │             │
      ├──────────────────────┼────────────────────┼─────────────────┼─────────────────▶│             │
      │                      │                    │                 │                  │             │
      │                      │                    │                 │  Verify 6 confs  │             │
      │                      │                    │                 │◀─────────────────┤             │
      │                      │                    │                 │                  │             │
      │                      │                    │                 │                  │  Verify SPV │
      │                      │                    │                 │                  │─────┐       │
      │                      │                    │                 │                  │     │       │
      │                      │                    │                 │                  │◀────┘       │
      │                      │                    │                 │                  │             │
      │                      │                    │                 │                  │  mint()     │
      │                      │                    │                 │                  ├────────────▶│
      │                      │                    │                 │                  │             │
      │                      │                    │                 │                  │ emit        │
      │                      │                    │                 │                  │ UtxoRegistered
      │                      │                    │                 │                  │──────────┐  │
      │                      │                    │                 │                  │          │  │
      │                      │                    │  ┌─────────────────────────────────────────────┐ │
      │                      │                    │  │ Off-chain Indexer listens:                  │ │
      │                      │                    │  │ UtxoRegistered(utxoId, txid, vout,          │ │
      │                      │                    │  │                amount, DEPOSIT)             │ │
      │                      │                    │  │ → Add UTXO to available pool                │ │
      │                      │                    │  └─────────────────────────────────────────────┘ │
      │                      │                    │                 │                  │             │
      │◀─────────────────────────────────────────────────────────────────────────────────────────────┤
      │  wBTC Tokens         │                    │                 │                  │             │
      │                      │                    │                 │                  │             │
```

## Withdrawal Flow (UTXO Indexer)

```
    User     Indexer API   BridgeGateway    WBTC    Operator1  Operator2  OperatorM  L2-Watcher  Bitcoin L1
      │            │              │           │           │          │          │          │            │
      │            │              │           │           │          │          │          │            │
┌─────┴────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 1. User Queries Indexer API for Available UTXOs                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
      │            │              │           │           │          │          │          │            │
      │ GET /utxos/select?        │           │           │          │          │          │            │
      │ amount=25000&address=...  │           │           │          │          │          │            │
      ├───────────▶│              │           │           │          │          │          │            │
      │            │              │           │           │          │          │          │            │
      │            │ Query event-sourced      │           │          │          │          │            │
      │            │ UTXO pool (UtxoRegistered - UtxoSpent)          │          │          │            │
      │            │ LARGEST_FIRST selection  │           │          │          │          │            │
      │            │              │           │           │          │          │          │            │
      │◀───────────┤              │           │           │          │          │          │            │
      │ {selected: [{utxoId, txid, vout, amount}]}        │          │          │          │            │
      │            │              │           │           │          │          │          │            │
┌─────┴────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 2. User Requests Withdrawal with Selected UTXOs                                                      │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
      │            │              │           │           │          │          │          │            │
      │ requestWithdraw(amount, destSpk, deadline, selectedUtxoIds)  │          │          │            │
      ├────────────┼─────────────▶│           │           │          │          │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ Validate UTXOs:       │          │          │          │            │
      │            │              │ • Check utxoSpent[id] == false   │          │          │            │
      │            │              │ • Check utxoSource[id] == DEPOSIT│          │          │            │
      │            │              │ • Check total amount sufficient  │          │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ Lock wBTC (transferFrom user → bridge)      │          │            │
      │            │              ├──────────▶│           │          │          │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ Store selectedUtxoIds (NOT marked spent yet)│          │            │
      │            │              │ (Will be marked in Step 4)       │          │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ Construct PSBT with selectedUtxoIds         │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ emit WithdrawalInitiated(wid, user, signerSetId, deadline, outputsHash, psbt)
      │            │              │─ ─ ─ ─ ─ ─ ─ ─(broadcast event)─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │            │
      │            │              │           │           │          │          │          │            │
      │            │  ┌───────────────────────────────────────────────────────────────────────────────┐ │
      │            │  │ Indexer listens: WithdrawalInitiated (includes PSBT)                          │ │
      │            │  │ • Parse PSBT to extract: wid, amount, destSpk, selectedUtxoIds                │ │
      │            │  │ • Mark as "pending" (not yet removed from available pool)                     │ │
      │            │  │ • Wait for UtxoSpent event (Step 4) to confirm spent status                   │ │
      │            │  └───────────────────────────────────────────────────────────────────────────────┘ │
      │            │              │           │           │          │          │          │            │
┌─────┴────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 3. Operators Listen, Build Bitcoin TX, Sign EIP-712 (Off-chain or On-chain)                          │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
      │            │              │           │           │          │          │          │            │
      │            │  ┌───────────────────────────────────────────────────────────────────────────────┐ │
      │            │  │ Operator1, Operator2, ..., OperatorM listen: WithdrawalInitiated (includes PSBT)│
      │            │  │ • Each operator independently parses PSBT to extract:                         │ │
      │            │  │   - selectedUtxoIds (inputs)                                                  │ │
      │            │  │   - dest + change + anchor (outputs)                                          │ │
      │            │  │ • Each operator signs EIP-712 digest: WithdrawApproval(wid, outputsHash, ...) │ │
      │            │  │                                                                               │ │
      │            │  │ TWO OPTIONS:                                                                  │ │
      │            │  │ A) Incremental: Each operator calls submitSignature(wid, sig, rawTx)          │ │
      │            │  │    → Contract stores sigs, auto-finalizes when M-th sig received              │ │
      │            │  │ B) Batch: Off-chain coordination → Collect M sigs → One                       │ │
      │            │  │    submits all at once via finalizeByApprovals()                              │ │
      │            │  └───────────────────────────────────────────────────────────────────────────────┘ │
      │            │              │           │           │          │          │          │            │
┌─────┴────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 4A. INCREMENTAL SIGNING (No Coordination)                                                            │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
      │            │              │           │           │          │          │          │            │
      │            │              │ submitSignature(wid, sig1, "")              │          │            │
      │            │              │◀──────────┤           │          │          │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ ┌─────────────────────────────────────────┐ │          │            │
      │            │              │ │ Verify sig1 (ECDSA recover)             │ │          │            │
      │            │              │ │ Store sig1 → signatureBitmap |= bit1    │ │          │            │
      │            │              │ │ signatureCount = 1                      │ │          │            │
      │            │              │ │ State: Pending (threshold not reached)  │ │          │            │
      │            │              │ └─────────────────────────────────────────┘ │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ emit SignatureSubmitted(wid, operator1, idx1)          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │           │ submitSignature(wid, sig2, "")  │          │            │
      │            │              │           │◀──────────┤          │          │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ ┌─────────────────────────────────────────┐ │          │            │
      │            │              │ │ Verify sig2                             │ │          │            │
      │            │              │ │ Store sig2 → signatureBitmap |= bit2    │ │          │            │
      │            │              │ │ signatureCount = 2                      │ │          │            │
      │            │              │ └─────────────────────────────────────────┘ │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ emit SignatureSubmitted(wid, operator2, idx2)          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │           │           │  ... (repeat until M-th)       │            │
      │            │              │           │           │          │          │          │            │
      │            │              │           │           │          │  submitSignature(wid, sigM, rawTx)
      │            │              │           │           │          │◀─────────┤          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ ┌─────────────────────────────────────────┐ │          │            │
      │            │              │ │ Verify sigM                             │ │          │            │
      │            │              │ │ Store sigM → signatureBitmap |= bitM    │ │          │            │
      │            │              │ │ signatureCount = M = threshold!         │ │          │            │
      │            │              │ │ State: Ready                            │ │          │            │
      │            │              │ └─────────────────────────────────────────┘ │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ emit SignatureSubmitted(wid, operatorM, idxM)          │            │
      │            │              │ emit WithdrawalReady(wid, user, amount, destSpk)       │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ ┌─────────────────────────────────────────┐ │          │            │
      │            │              │ │ AUTO-FINALIZE (if rawTx provided):      │ │          │            │
      │            │              │ │ • Verify rawTx outputs match policy     │ │          │            │
      │            │              │ │ • Mark UTXOs as spent                   │ │          │            │
      │            │              │ │ • Atomic burn wBTC                      │ │          │            │
      │            │              │ │ • Emit SignedTxReady                    │ │          │            │
      │            │              │ └─────────────────────────────────────────┘ │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ emit UtxoSpent(utxoId, wid, ...) for each UTXO         │            │
      │            │              │─ ─ ─ ─ ─ ─ ─ ─(broadcast event)─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │            │
      │            │  ┌───────────────────────────────────────────────────────────────────────────────┐ │
      │            │  │ Indexer listens: UtxoSpent → Remove UTXO from available pool                  │ │
      │            │  └───────────────────────────────────────────────────────────────────────────────┘ │
      │            │              │           │           │          │          │          │            │
      │            │              │ Atomic burn wBTC      │          │          │          │            │
      │            │              ├──────────▶│           │          │          │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ emit SignedTxReady(wid, user, txid, amount, rawTx)     │            │
      │            │              │─ ─ ─ ─ ─ ─ ─ ─(broadcast event)─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │            │
      │            │              │           │           │          │          │          │            │
┌─────┴────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 4B. BATCH FINALIZATION (Alternative Flow - Gas Optimized, Requires Off-chain Coordination)           │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
      │            │              │           │           │          │          │          │            │
      │            │  ┌───────────────────────────────────────────────────────────────────────────────┐ │
      │            │  │ Operators coordinate off-chain:                                               │ │
      │            │  │ • All M operators sign EIP-712 approval digest                                │ │
      │            │  │ • Collect M signatures in bitmap order                                        │ │
      │            │  │ • One operator aggregates and submits all at once                             │ │
      │            │  └───────────────────────────────────────────────────────────────────────────────┘ │
      │            │              │           │           │          │          │          │            │
      │            │              │ finalizeByApprovals(wid, rawTx, outputsHash, signerBitmap, sigs[M]) │
      │            │              │◀──────────┤ (Any party submits batch of M signatures)  │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ ┌─────────────────────────────────────────┐ │          │            │
      │            │              │ │ Verify M-of-N EIP-712 signatures        │ │          │            │
      │            │              │ │ Verify rawTx outputs match policy       │ │          │            │
      │            │              │ │ → If valid, proceed to finalization     │ │          │            │
      │            │              │ └─────────────────────────────────────────┘ │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ Mark UTXOs as spent:  │          │          │          │            │
      │            │              │ utxoSpent[id] = true  │          │          │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ Atomic burn wBTC      │          │          │          │            │
      │            │              ├──────────▶│           │          │          │          │            │
      │            │              │           │           │          │          │          │            │
      │            │              │ emit UtxoSpent(utxoId, wid, ...) for each UTXO         │            │
      │            │              │─ ─ ─ ─ ─ ─ ─ ─(broadcast event)─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │            │
      │            │  ┌───────────────────────────────────────────────────────────────────────────────┐ │
      │            │  │ Indexer listens: UtxoSpent → Remove UTXO from available pool                  │ │
      │            │  └───────────────────────────────────────────────────────────────────────────────┘ │
      │            │              │           │           │          │          │          │            │
      │            │              │ emit SignedTxReady(wid, user, txid, amount, rawTx)     │            │
      │            │              │─ ─ ─ ─ ─ ─ ─ ─(broadcast event)─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │            │
      │            │              │           │           │          │          │          │            │
┌─────┴────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 5. L2-Watcher Broadcasts Signed TX to Bitcoin Network (Off-chain)                                    │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
      │            │              │           │           │          │          │          │            │
      │            │  ┌───────────────────────────────────────────────────────────────────────────────┐ │
      │            │  │ L2-Watcher listens: SignedTxReady event                                       │ │
      │            │  │ • Extract rawTx from event                                                    │ │
      │            │  │ • Broadcast to Bitcoin network via RPC                                        │ │
      │            │  │ • Monitor 6+ confirmations                                                    │ │
      │            │  └───────────────────────────────────────────────────────────────────────────────┘ │
      │            │              │           │           │          │          │          │            │
      │            │              │           │           │          │          │  bitcoin-cli sendrawtransaction
      │            │              │           │           │          │          │──────────────────────▶│
      │            │              │           │           │          │          │          │            │
      │            │              │           │           │          │          │ Monitor confirmations │
      │            │              │           │           │          │          │◀──────────────────────┤
      │            │              │           │           │          │          │          │            │
      │            │              │           │           │          │          │ 6+ confirms           │
      │            │              │           │           │          │          │          │            │
┌─────┴────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 6. Withdrawal Complete (L2 finalized at Step 4, L1 confirmed after 6 blocks)                         │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
      │            │              │           │           │          │          │          │            │
      │            │              │           │           │          │          │          │            │
      │            │ L2 State: Finalized (wBTC burned, UTXOs marked spent)      │          │            │
      │            │ L1 State: Transaction confirmed with 6+ blocks             │          │            │
      │            │              │           │           │          │          │          │            │
```

## State Machine: Withdrawal Lifecycle

```
                    requestWithdraw()
                           │
                           ▼
                   ┌───────────────┐
                   │     None      │
                   └───────────────┘
                           │
                           ▼
                   ┌───────────────┐
              ┌───▶│   Pending     │◀───┐
              │    └───────────────┘    │
              │            │            │
              │            │ submitSignature()
              │            │ (Individual)
              │            ▼            │
              │    ┌───────────────┐    │
              │    │     Ready     │────┘
              │    │ (Threshold!)  │
              │    └───────────────┘
              │            │
              │            │ finalizeByApprovals()
              │            │ or auto-finalize
              │            ▼
              │    ┌───────────────┐
              │    │  Finalized    │
              │    │ (wBTC burned) │
              │    └───────────────┘
              │
              │ cancelWithdraw()
              │ (before deadline)
              │
              │    ┌───────────────┐
              └────│   Canceled    │
                   │ (wBTC refund) │
                   └───────────────┘
```

## Data Flow: Deposit SPV Proof

```
┌─────────────────────────────────────────────────────────────┐
│                   Bitcoin Transaction                       │
│                                                             │
│  Inputs: [...]                                              │
│  Outputs:                                                   │
│    [0] Vault: 50000 sats                                    │
│    [1] OP_RETURN: <envelope>                                │
│         ├─ Tag: "MJVB" (4 bytes)                            │
│         ├─ Chain ID: 1729 (32 bytes)                        │
│         ├─ Bridge: 0x67CE...C37B (20 bytes)                 │
│         ├─ Recipient: 0xf39F...2266 (20 bytes)              │
│         └─ Amount: 50000 (32 bytes)                         │
│                                                             │
│  TXID: 0xabcd...1234                                        │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ Included in Block
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                   Bitcoin Block #107                      │
│                                                             │
│  Header (80 bytes):                                         │
│    ├─ Version: 0x20000000                                   │
│    ├─ Prev Block: 0x...                                     │
│    ├─ Merkle Root: 0xabc...def                              │
│    ├─ Timestamp: 1234567890                                 │
│    ├─ Bits: 0x1d00ffff                                      │
│    └─ Nonce: 0x12345678                                     │
│                                                             │
│  Merkle Tree:                                               │
│         Root: 0xabc...def                                   │
│        /              \                                     │
│   0x123...         0x456...                                 │
│    /    \           /    \                                  │
│  [TX0] [TX1]     [TX2]  [TX3]                               │
│           │                                                 │
│           └─ Our TX: 0xabcd...1234 (index=1)                │
│                                                             │
│  Merkle Proof: [0x123..., 0x456...]                         │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ SPV Proof
                          ▼
┌─────────────────────────────────────────────────────────────┐
│              claimDepositSpv() Parameters                   │
│                                                             │
│  recipient: 0xf39F...2266                                   │
│  amountSats: 50000                                          │
│  envelopeHash: keccak256(envelope)                          │
│                                                             │
│  SpvProof:                                                  │
│    ├─ rawTx: <serialized TX>                                │
│    ├─ txid: 0xabcd...1234                                   │
│    ├─ merkleBranch: [0x123..., 0x456...]                    │
│    ├─ index: 1                                              │
│    ├─ header0: <80 byte header>                             │
│    └─ confirmHeaders: []                                    │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
                  BridgeGateway Verification:
                    1. BtcRelay.verifyConfirmations(6) ✓
                    2. Verify Merkle Proof ✓
                    3. Parse envelope from OP_RETURN ✓
                    4. Validate envelope hash ✓
                    5. Check duplicate (outpoint) ✓
                          │
                          ▼
                    WBTC.mint(recipient, 50000)
```

## Security: Multi-signature Validation

```
┌──────────────────────────────────────────────────────────────┐
│            Operator Set Configuration (M-of-N)               │
│                                                              │
│  Threshold: M = 4                                            │
│  Total Operators: N = 5                                      │
│                                                              │
│  Operators:                                                  │
│    [0] 0x1234...  ─────┐                                     │
│    [1] 0x5678...  ─────┤                                     │
│    [2] 0xabcd...  ─────┼── Required: Any 4 of 5              │
│    [3] 0xef01...  ─────┤                                     │
│    [4] 0x2345...  ─────┘                                     │
└──────────────────────────────────────────────────────────────┘
                          │
                          │ Withdrawal Request
                          ▼
┌──────────────────────────────────────────────────────────────┐
│                  Signature Collection                        │
│                                                              │
│  EIP-712 Approval Digest:                                    │
│    ├─ Domain: BridgeGateway, Chain 1729                      │
│    ├─ TypeHash: WithdrawApproval(...)                        │
│    └─ Data:                                                  │
│        ├─ wid: 0x88ee...                                     │
│        ├─ outputsHash: 0xace9...                             │
│        ├─ version: 1                                         │
│        ├─ expiry: 1764143598                                 │
│        └─ signerSetId: 1                                     │
│                                                              │
│  Digest: 0xbb24fbdc...                                       │
└──────────────────────────────────────────────────────────────┘
                          │
                          │ Sign with Private Keys
                          ▼
┌──────────────────────────────────────────────────────────────┐
│               Signature Verification Process                 │
│                                                              │
│  Operator[0]: Sign → 0xf349...1b ✓ Valid                     │
│  Operator[1]: Sign → 0xff24...1c ✓ Valid                     │
│  Operator[2]: Sign → 0xfc91...1c ✓ Valid                     │
│  Operator[3]: Sign → 0x9b40...1b ✓ Valid                     │
│                                                              │
│  Signature Bitmap: 0b1111 = 15                               │
│    [0][1][2][3][4]                                           │
│     1  1  1  1  0  ← Operators who signed                    │
│                                                              │
│  Collected: 4 signatures                                     │
│  Threshold: 4 required                                       │
│                                                              │
│  Status: ✓ THRESHOLD REACHED                                 │
└──────────────────────────────────────────────────────────────┘
                          │
                          ▼
                  Withdrawal Finalized
                  wBTC Burned
                  Bitcoin TX Ready
```

## Events Timeline

```
Time    Event                           State           Data
─────────────────────────────────────────────────────────────────────────────────
T0      WithdrawalInitiated             Pending         wid, user, signerSetId,
        (Optimized Single Event)                        deadline, outputsHash, psbt
        │                                               (psbt contains: amount, destSpk, UTXOs)
        │                                               (~2K gas saved vs dual events)
        │
        ├─ Indexer parses PSBT to extract withdrawal details
        ├─ Validator1 listens & parses PSBT
        │
T1      SignatureSubmitted              Pending         wid, validator1, sig
        │  (1/4)                                        (Optional: incremental signing)
        │
T2      SignatureSubmitted              Pending         wid, validator2, sig
        │  (2/4)
        │
T3      SignatureSubmitted              Pending         wid, validator3, sig
        │  (3/4)
        │
T4      SignatureSubmitted              Pending→Ready   wid, validator4, sig
        │  (4/4 - Threshold!)
        │
T4      WithdrawalReady                 Ready           wid, user, amount, destSpk
        │
        ├─ L2-Watcher listens
        │
T5      [Operators coordinate M-of-N signatures off-chain]
        │
T6      finalizeByApprovals()           Pending→        wid, rawTx, sigs[]
        │  (Primary flow)               Finalized       (Batch verification)
        │
T7      UtxoSpent (for each UTXO)       -               utxoId, wid
        │  (Event-sourced state update)
        │
T8      SignedTxReady                   Finalized       wid, txid, rawTx
        │  (= WithdrawalSucceed)
        │  wBTC burned atomically
        │
        ├─ L2-Watcher broadcasts rawTx to Bitcoin
        │
T9      [Bitcoin confirms TX]
        │
        └─ Withdrawal complete
```
