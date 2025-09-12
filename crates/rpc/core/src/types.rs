use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Namespace {
    #[serde(rename = "debug")]
    Debug,
    #[serde(rename = "eth")]
    Eth,
    #[serde(rename = "moj")]
    Mojave,
    #[serde(rename = "net")]
    Net,
    #[serde(rename = "txpool")]
    TxPool,
    #[serde(rename = "web3")]
    Web3,
}

#[derive(Eq, PartialEq, Serialize, Deserialize)]
pub enum MojaveRequestMethods {
    #[serde(rename = "moj_getJobId")]
    GetJobId,
    #[serde(rename = "moj_getProof")]
    GetProof,
    #[serde(rename = "moj_sendProofInput")]
    SendProofInput,
}
