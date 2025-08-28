use ethrex_common::types::{Block, BlockBody, Transaction};
use ethrex_rpc::{
    RpcErr,
    types::{block::RpcBlock, block_identifier::BlockIdentifier},
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::rpc::{RpcApiContext, types::OrderedBlock};

pub(crate) async fn ingest_block(context: RpcApiContext, block_number: u64) -> Result<(), RpcErr> {
    let Some(peeked) = context.pending_signed_blocks.peek().await else {
        return Err(RpcErr::Internal(
            "No pending signed blocks, no ingestion needed".into(),
        ));
    };

    if block_number == peeked.0.header.number {
        // Push the signed block from the pending queue to the block queue.
        let signed_block =
            context.pending_signed_blocks.pop().await.ok_or_else(|| {
                RpcErr::Internal("Pending queue became empty while ingesting".into())
            })?;

        context.block_queue.push(signed_block).await;
        return Ok(());
    }

    // Back fill missing block
    let rpc_block = context
        .eth_client
        .get_block_by_number(BlockIdentifier::Number(block_number))
        .await
        .map_err(|e| RpcErr::Internal(e.to_string()))?;

    let block = rpc_block_to_block(rpc_block);
    context.block_queue.push(OrderedBlock(block)).await;

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

pub(crate) fn spawn_block_ingestion_task(
    context: RpcApiContext,
    shutdown_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let mut current_block_number =
            match context.l1_context.storage.get_latest_block_number().await {
                Ok(num) => num.saturating_add(1),
                Err(_) => {
                    tracing::error!("Failed to get latest block number from storage");
                    1
                }
            };

        tracing::info!("Starting block ingestion loop @ {current_block_number}");
        loop {
            tokio::select! {
                result = ingest_block(context.clone(), current_block_number) => {
                    match result {
                        Ok(()) => {
                            current_block_number += 1;
                            tracing::info!("Ingested block number: {}", current_block_number - 1);
                        },
                        Err(error) => {
                            tracing::error!("Failed to ingest a block: {}", error);
                        }
                    };
                }
                _ = shutdown_token.cancelled() => {
                    tracing::info!("Shutting down block ingestion loop");
                    break;
                }
            }
        }
    })
}