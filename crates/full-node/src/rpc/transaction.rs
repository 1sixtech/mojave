use crate::rpc::RpcApiContext;
use ethrex_rpc::{RpcErr, utils::RpcRequest};
use serde_json::Value;

pub struct SendRawTransactionRequest;

impl SendRawTransactionRequest {
    pub async fn call(request: &RpcRequest, context: RpcApiContext) -> Result<Value, RpcErr> {
        let data = get_transaction_data(&request.params)?;
        // let tx_hash = context.mojave_client.
        let tx_hash = Value::Null;
        Ok(tx_hash)
    }
}

fn get_transaction_data(rpc_req_params: &Option<Vec<Value>>) -> Result<Vec<u8>, RpcErr> {
    let params = rpc_req_params
        .as_ref()
        .ok_or(RpcErr::BadParams("No params provided".to_owned()))?;
    if params.len() != 1 {
        return Err(RpcErr::BadParams(format!(
            "Expected one param and {} were provided",
            params.len()
        )));
    };

    let str_data = serde_json::from_value::<String>(params[0].clone())?;
    let str_data = str_data
        .strip_prefix("0x")
        .ok_or(RpcErr::BadParams("Params are not 0x prefixed".to_owned()))?;
    hex::decode(str_data).map_err(|error| RpcErr::BadParams(error.to_string()))
}
