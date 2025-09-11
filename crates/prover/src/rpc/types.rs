use mojave_client::types::ProverData;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendProofInputRequest(ProverData);

pub use crate::job::JobRecord;
