use ethrex_l2_common::prover::BatchProof;
use serde::{Deserialize, Serialize};
use zkvm_interface::io::ProgramInput;

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
