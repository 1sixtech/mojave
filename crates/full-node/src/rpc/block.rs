use crate::rpc::{RpcApiContext, types::OrderedBlock};
use ethrex_rpc::{RpcErr, utils::RpcRequest};
use mojave_client::types::SignedBlock;
use mojave_signature::Verifier;
use serde_json::Value;

pub struct SendBroadcastBlockRequest {
    signed_block: SignedBlock,
}

impl SendBroadcastBlockRequest {
    fn get_block_data(req: &Option<Vec<Value>>) -> Result<Self, RpcErr> {
        let params = req
            .as_ref()
            .ok_or(RpcErr::BadParams("No params provided".to_owned()))?;

        if params.len() != 1 {
            return Err(RpcErr::BadParams(format!(
                "Expected exactly 1 parameter (SignedBlock), but {} were provided",
                params.len()
            )));
        }

        let signed_block_param = params.first().ok_or(RpcErr::BadParams(
            "Missing SignedBlock parameter".to_owned(),
        ))?;

        let signed_block = serde_json::from_value::<SignedBlock>(signed_block_param.clone())?;
        Ok(Self { signed_block })
    }

    pub async fn call(request: &RpcRequest, context: RpcApiContext) -> Result<Value, RpcErr> {
        let data = Self::get_block_data(&request.params)?;

        // Check if the signature and sender are valid. If verification fails, return an error
        // immediately without processing the block.
        data.signed_block
            .verifying_key
            .verify(
                &data.signed_block.block.header.hash(),
                &data.signed_block.signature,
            )
            .map_err(|error| RpcErr::Internal(error.to_string()))?;

        let signed_block = data.signed_block.block;
        let signed_block_number = signed_block.header.number;

        // Push the signed block to the block queue for processing
        context
            .pending_signed_blocks
            .push(OrderedBlock(signed_block.clone()))
            .await;

        tracing::info!("Received the block number: {}", signed_block_number);
        Ok(Value::Null)
    }
}
