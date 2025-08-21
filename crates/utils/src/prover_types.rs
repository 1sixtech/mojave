use ethrex_l2_common::prover::BatchProof;
use serde::{Deserialize, Serialize};
use zkvm_interface::io::ProgramInput;

#[derive(Deserialize, Serialize)]
pub struct ProverData {
    pub batch_number: u64,
    pub input: ProgramInput,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ProofResponse {
    pub job_id: String,
    pub batch_number: u64,
    pub error: Option<String>,
    pub batch_proof: Option<BatchProof>,
}
