use crate::rpc::context::RpcApiContext;
use mojave_utils::rpc::error::{Error, Result};
use serde_json::Value;

pub async fn send_raw_transaction(ctx: &RpcApiContext, raw: Vec<u8>) -> Result<Value> {
    let tx_hash = ctx
        .eth_client
        .send_raw_transaction(&raw)
        .await
        .map_err(|error| Error::Internal(error.to_string()))?;
    serde_json::to_value(tx_hash).map_err(|error| Error::Internal(error.to_string()))
}
