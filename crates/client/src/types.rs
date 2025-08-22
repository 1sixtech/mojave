use ethrex_common::types::Block;
use ethrex_l2_common::prover::BatchProof;
use mojave_signature::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use zkvm_interface::io::ProgramInput;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobId(String);

#[derive(Deserialize, Serialize)]
pub struct ProverData {
    pub batch_number: u64,
    pub input: ProgramInput,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProofResponse {
    pub job_id: String,
    pub batch_number: u64,
    pub result: ProofResult,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ProofResult {
    Proof(BatchProof),
    Error(String),
}
