use mojave_client::types::ProverData;
use reqwest::Url;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendProofInputRequest {
    pub prover_data: ProverData,
    pub sequencer_addr: Url,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SendProofInputParam {
    Object(SendProofInputRequest),
    Tuple((ProverData, Url)),
}

pub use crate::job::JobRecord;
