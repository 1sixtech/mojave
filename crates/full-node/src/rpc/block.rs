use crate::rpc::RpcApiContext;
use ethrex_rpc::{RpcErr, utils::RpcRequest};
use serde_json::Value;

pub struct SendBroadcastBlockRequest;

impl SendBroadcastBlockRequest {
    pub async fn call(request: &RpcRequest, context: RpcApiContext) -> Result<Value, RpcErr> {
        Ok(Value::Null)
    }
}
