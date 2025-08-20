use crate::rpc::{RpcApiContext, types::OrderedBlock};
use ethrex_common::types::{Block, BlockBody, Transaction};
use ethrex_rpc::{
    RpcErr,
    types::{block::RpcBlock, block_identifier::BlockIdentifier},
    utils::RpcRequest,
};
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

        let latest_block_number = context.l1_context.storage.get_latest_block_number().await? + 1;
        let signed_block_number = data.signed_block.block.header.number;
        for block_number in latest_block_number..signed_block_number {
            let block = context
                .eth_client
                .get_block_by_number(BlockIdentifier::Number(block_number))
                .await
                .map_err(|error| RpcErr::Internal(error.to_string()))?;
            let block = rpc_block_to_block(block);
            context.block_queue.push(OrderedBlock(block)).await;
        }

        context
            .block_queue
            .push(OrderedBlock(data.signed_block.block))
            .await;
        tracing::info!("Received the block number: {}", signed_block_number);
        Ok(Value::Null)
    }
}

fn rpc_block_to_block(rpc_block: RpcBlock) -> Block {
    match rpc_block.body {
        ethrex_rpc::types::block::BlockBodyWrapper::Full(full_block_body) => {
            // transform RPCBlock to normal block
            let transactions: Vec<Transaction> = full_block_body
                .transactions
                .iter()
                .map(|b| b.tx.clone())
                .collect();

            Block::new(
                rpc_block.header,
                BlockBody {
                    ommers: vec![],
                    transactions,
                    withdrawals: Some(full_block_body.withdrawals),
                },
            )
        }
        ethrex_rpc::types::block::BlockBodyWrapper::OnlyHashes(..) => {
            unreachable!()
        }
    }
}
