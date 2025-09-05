use ethrex_common::types::Block;
use ethrex_l2_common::prover::BatchProof;
use guest_program::input::ProgramInput;
use mojave_signature::{VerifyingKey, types::Signature};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub enum Strategy {
    /// Create sequential RPC requests that returns a first succesful response or an error if all requests fail.
    Sequential,
    /// Sends multiple RPC requests to a list of urls and returns the first response without waiting for others to finish.
    Race,
}

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
