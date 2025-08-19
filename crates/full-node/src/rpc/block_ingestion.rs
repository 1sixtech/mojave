use ethrex_common::types::{Block, BlockBody, Transaction};
use ethrex_rpc::{
    EthClient, RpcErr,
    types::{block::RpcBlock, block_identifier::BlockIdentifier},
};
use mojave_chain_utils::unique_heap::AsyncUniqueHeap;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

use crate::rpc::types::OrderedBlock;

#[derive(Debug)]
pub struct BlockIngestion {
    next_expected: u64,
}

impl BlockIngestion {
    pub fn new(start: u64) -> Self {
        Self {
            next_expected: start,
        }
    }

    pub fn next_expected(&self) -> u64 {
        self.next_expected
    }

    fn advance(&mut self, new: u64) {
        if new > self.next_expected {
            self.next_expected = new;
        }
    }

    pub async fn ingest_block(
        ingestion: &Arc<TokioMutex<BlockIngestion>>,
        eth_client: &EthClient,
        block_queue: &AsyncUniqueHeap<OrderedBlock, u64>,
        signed_block: Block,
    ) -> Result<(), RpcErr> {
        let signed_block_number = signed_block.header.number;

        // ---- lock to read state
        let guard = ingestion.lock().await;
        let expected = guard.next_expected();

        // already processed or behind: skip quickly
        if signed_block_number < expected {
            tracing::debug!(
                "Skipping block {}, next_expected={}",
                signed_block_number,
                expected
            );
            return Ok(());
        }

        // release lock before doing network I/O
        drop(guard);

        // ---- backfill any missing blocks
        if signed_block_number > expected {
            for number in expected..signed_block_number {
                let rpc_block: RpcBlock = eth_client
                    .get_block_by_number(BlockIdentifier::Number(number))
                    .await
                    .map_err(|e| RpcErr::Internal(e.to_string()))?;

                let block = rpc_block_to_block(rpc_block);
                block_queue.push(OrderedBlock(block)).await;
            }
        }

        // ---- push the provided signed block
        block_queue.push(OrderedBlock(signed_block)).await;

        // ---- lock again to advance state (only after successful ingestion)
        let mut guard = ingestion.lock().await;
        // ensure monotonic progress in case another task advanced meanwhile
        guard.advance(signed_block_number + 1);

        Ok(())
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
