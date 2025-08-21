use ethrex_common::types::Block;
use mojave_signature::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use mojave_chain_utils::prover_types::ProofResponse;

// need to check whether we will use Message and contain other data or not
#[derive(Serialize, Deserialize)]
pub struct SignedBlock {
    pub block: Block,
    pub signature: Signature,
    pub verifying_key: VerifyingKey,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SignedProofResponse {
    pub proof_response: ProofResponse,
    pub signature: Signature,
    pub verifying_key: VerifyingKey,
}
