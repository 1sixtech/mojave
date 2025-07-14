use crate::rpc::{RpcHandler, utils::RpcErr};

use super::RpcApiContextSequencer;
use ethrex_rpc::types::transaction::SendRawTransactionRequest;
use serde_json::Value;

impl RpcHandler<RpcApiContextSequencer> for SendRawTransactionRequest {
    fn parse(params: &Option<Vec<Value>>) -> Result<SendRawTransactionRequest, RpcErr> {
        let data = get_transaction_data(params)?;
        let transaction = SendRawTransactionRequest::decode_canonical(&data)
            .map_err(|error| RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(error.to_string())))?;
        if matches!(transaction, SendRawTransactionRequest::PrivilegedL2(_)) {
            return Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(
                "Invalid transaction type".to_string(),
            )));
        }

        Ok(transaction)
    }

    async fn handle(&self, context: RpcApiContextSequencer) -> Result<Value, RpcErr> {
        let hash = if let SendRawTransactionRequest::EIP4844(wrapped_blob_tx) = self {
            context
                .l1_context
                .blockchain
                .add_blob_transaction_to_pool(
                    wrapped_blob_tx.tx.clone(),
                    wrapped_blob_tx.blobs_bundle.clone(),
                )
                .await
        } else {
            context
                .l1_context
                .blockchain
                .add_transaction_to_pool(self.to_transaction())
                .await
        }?;
        serde_json::to_value(format!("{hash:#x}"))
            .map_err(|error| RpcErr::EthrexRPC(ethrex_rpc::RpcErr::Internal(error.to_string())))
    }
}

fn get_transaction_data(rpc_req_params: &Option<Vec<Value>>) -> Result<Vec<u8>, RpcErr> {
    let params =
        rpc_req_params
            .as_ref()
            .ok_or(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(
                "No params provided".to_owned(),
            )))?;
    if params.len() != 1 {
        return Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(format!(
            "Expected one param and {} were provided",
            params.len()
        ))));
    };

    let str_data = serde_json::from_value::<String>(params[0].clone())?;
    let str_data =
        str_data
            .strip_prefix("0x")
            .ok_or(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(
                "Params are not 0x prefixed".to_owned(),
            )))?;
    hex::decode(str_data)
        .map_err(|error| RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(error.to_string())))
}
