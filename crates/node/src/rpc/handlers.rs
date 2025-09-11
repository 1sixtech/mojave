use crate::{rpc::context::RpcApiContext, services::block::ingest_signed_block};
use mojave_client::types::SignedBlock;

#[mojave_rpc_macros::rpc(namespace = "moj", method = "sendBroadcastBlock")]
pub async fn broadcast_block(
    ctx: RpcApiContext,
    params: SignedBlock,
) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
    ingest_signed_block(&ctx, params).await?;
    Ok(serde_json::Value::Null)
}
