use crate::{rpc::RpcApiContext, services::proof::accept_signed_proof};
use mojave_client::types::SignedProofResponse;

#[mojave_rpc_macros::rpc(namespace = "moj", method = "sendProofResponse")]
pub async fn send_proof_response(
    ctx: RpcApiContext,
    params: SignedProofResponse,
) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
    accept_signed_proof(&ctx, params).await?;
    Ok(serde_json::json!("Proof accepted"))
}
