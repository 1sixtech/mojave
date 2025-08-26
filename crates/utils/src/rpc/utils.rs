use crate::rpc::{
    error::Result,
    types::{RpcErrorResponse, RpcRequestId, RpcSuccessResponse},
};
use serde_json::Value;

pub fn rpc_response(id: RpcRequestId, res: Result<Value>) -> Result<Value> {
    Ok(match res {
        Ok(result) => serde_json::to_value(RpcSuccessResponse {
            id,
            jsonrpc: "2.0".to_string(),
            result,
        }),
        Err(error) => {
            tracing::error!("RPC error: {:?}", error);
            serde_json::to_value(RpcErrorResponse {
                id,
                jsonrpc: "2.0".to_string(),
                error: error.into(),
            })
        }
    }?)
}
