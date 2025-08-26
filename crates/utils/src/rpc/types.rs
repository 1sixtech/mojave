pub use ethrex_rpc::utils::{RpcErrorResponse, RpcRequest, RpcRequestId, RpcSuccessResponse};
use serde::{Deserialize, Serialize};

// e.g. https://docs.nethermind.io/interacting/json-rpc-ns/admin/
#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub enum MojaveRequestMethods {
    #[serde(rename = "moj_sendBroadcastBlock")]
    SendBroadcastBlock,
    #[serde(rename = "moj_sendProofInput")]
    SendProofInput,
    #[serde(rename = "moj_sendProofResponse")]
    SendProofResponse,
    #[serde(rename = "moj_getJobId")]
    GetJobId,
    #[serde(rename = "moj_getProof")]
    GetProof,
}
