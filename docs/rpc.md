**Scope**

This document lists the Mojave-specific JSON-RPC methods from `MojaveRequestMethods` (`crates/rpc/core/src/types.rs`). Request/response schemas will be documented separately.

**Methods**

- `moj_sendProofInput` — Enqueue a proof-generation job with prover input and sequencer address. (Prover)
- `moj_getPendingJobIds` — List pending proof job IDs. (Prover)
- `moj_getProof` — Fetch the proof result for a given job ID. (Prover)
