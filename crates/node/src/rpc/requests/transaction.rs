use crate::rpc::RpcApiContext;
use mojave_utils::rpc::{
    error::{Error, Result},
    types::RpcRequest,
};
use serde_json::Value;

pub struct SendRawTransactionRequest(Vec<u8>);

impl SendRawTransactionRequest {
    fn get_transaction_data(rpc_req_params: &Option<Vec<Value>>) -> Result<Self> {
        let params = rpc_req_params
            .as_ref()
            .ok_or(Error::BadParams("No params provided".to_owned()))?;
        if params.len() != 1 {
            return Err(Error::BadParams(format!(
                "Expected one param and {} were provided",
                params.len()
            )));
        };

        let str_data = serde_json::from_value::<String>(params[0].clone())?;
        let str_data = str_data
            .strip_prefix("0x")
            .ok_or(Error::BadParams("Params are not 0x prefixed".to_owned()))?;
        let transaction_vec =
            hex::decode(str_data).map_err(|error| Error::BadParams(error.to_string()))?;
        Ok(Self(transaction_vec))
    }

    pub async fn call(request: &RpcRequest, context: RpcApiContext) -> Result<Value> {
        let data = Self::get_transaction_data(&request.params)?;
        let tx_hash = context
            .eth_client
            .send_raw_transaction(&data.0)
            .await
            .map_err(|error| Error::Internal(error.to_string()))?;
        serde_json::to_value(tx_hash).map_err(|error| Error::Internal(error.to_string()))
    }
}
