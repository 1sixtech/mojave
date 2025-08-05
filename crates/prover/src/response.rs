use ethrex_l2_common::prover::BatchProof;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum Response {
    Proof(BatchProof),
    Error(ResponseError),
}

#[derive(Serialize, Deserialize, Debug, thiserror::Error)]
pub(crate) enum ResponseError {
    #[error("Proof generate error: {0}")]
    ProofError(String),
    #[error("Error to read/write data from stream: {0}")]
    StreamError(String)
}