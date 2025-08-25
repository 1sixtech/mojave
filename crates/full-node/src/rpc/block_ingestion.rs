use ethrex_common::types::{Block, BlockBody, Transaction};
use ethrex_rpc::{
    RpcErr,
    types::{block::RpcBlock, block_identifier::BlockIdentifier},
};

use crate::rpc::{RpcApiContext, types::OrderedBlock};

pub(crate) async fn ingest_block(context: RpcApiContext, block_number: u64) -> Result<(), RpcErr> {
    let peek = context.pending_signed_blocks.peek().await;

    // peek must be not none now
    if peek.is_none() {
        return Err(RpcErr::Internal(
            "No pending signed blocks, no ingestion needed".to_string(),
        ));
    }

    if block_number != peek.unwrap().0.header.number {
        // Back fill missing block
        let rpc_block = context
            .eth_client
            .get_block_by_number(BlockIdentifier::Number(block_number))
            .await
            .map_err(|e| RpcErr::Internal(e.to_string()))?;

        let block = rpc_block_to_block(rpc_block);

        context.block_queue.push(OrderedBlock(block)).await;
    } else {
        // Push the signed block from pending queue to block queue
        let signed_block = context.pending_signed_blocks.pop().await.unwrap();

        context.block_queue.push(signed_block).await;
    }

    Ok(())
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
