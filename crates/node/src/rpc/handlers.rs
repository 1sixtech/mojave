use crate::{
    rpc::context::RpcApiContext,
    services::{block::ingest_signed_block, eth::send_raw_transaction},
};
use mojave_client::types::SignedBlock;

#[mojave_rpc_macros::rpc(namespace = "eth", method = "sendRawTransaction")]
pub async fn send_raw_tx(
    ctx: RpcApiContext,
    hex_tx: String,
) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
    let s = hex_tx.strip_prefix("0x").ok_or_else(|| {
        mojave_rpc_core::RpcErr::BadParams("Params are not 0x prefixed".to_owned())
    })?;
    let raw = hex::decode(s).map_err(|e| mojave_rpc_core::RpcErr::BadParams(e.to_string()))?;
    send_raw_transaction(&ctx, raw).await
}

#[mojave_rpc_macros::rpc(namespace = "moj", method = "sendBroadcastBlock")]
pub async fn broadcast_block(
    ctx: RpcApiContext,
    params: SignedBlock,
) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
    ingest_signed_block(&ctx, params).await?;
    Ok(serde_json::Value::Null)
}
