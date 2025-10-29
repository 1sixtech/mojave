use std::borrow::Borrow;

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
#[serde(deny_unknown_fields)]
pub struct SignedBlock {
    pub block: Block,
    pub signature: Signature,
    pub verifying_key: VerifyingKey,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SignedProofResponse {
    pub proof_response: ProofResponse,
    pub signature: Signature,
    pub verifying_key: VerifyingKey,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct JobId(String);

impl JobId {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Borrow<str> for JobId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for JobId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for JobId {
    fn from(s: String) -> Self {
        JobId(s)
    }
}

impl From<&str> for JobId {
    fn from(s: &str) -> Self {
        JobId(s.to_owned())
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProverData {
    pub batch_number: u64,
    pub input: ProgramInput,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProofResponse {
    pub job_id: JobId,
    pub batch_number: u64,
    pub result: ProofResult,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ProofResult {
    Proof(BatchProof),
    Error(String),
}
