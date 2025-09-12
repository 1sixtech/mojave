use ethrex_rpc::RpcErrorMetadata;
use serde::{Deserialize, Serialize};

use crate::RpcRequestId;

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
    #[serde(rename = "moj_getPendingJobIds")]
    GetPendingJobIds,
    #[serde(rename = "moj_getProof")]
    GetProof,
    #[serde(rename = "moj_sendProofInput")]
    SendProofInput,
}

#[derive(Serialize)]
pub struct RpcErrorResponse {
    pub jsonrpc: String,
    pub id: Option<RpcRequestId>,
    pub error: RpcErrorMetadata,
}
