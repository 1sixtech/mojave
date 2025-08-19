use std::sync::Arc;
use ethrex_common::types::{Block, BlockBody, Transaction};
use ethrex_rpc::{types::{block::RpcBlock, block_identifier::BlockIdentifier}, EthClient, RpcErr};
use mojave_chain_utils::unique_heap::AsyncUniqueHeap;
use tokio::sync::Mutex;

use crate::rpc::types::OrderedBlock;

#[derive(Debug)]
pub struct BlockIngestion {
    next_expected: u64,
}

impl BlockIngestion {
    pub fn new(start: u64) -> Self {
        Self { next_expected: start }
    }

    pub fn next_expected(&self) -> u64 {
        self.next_expected
    }

    fn advance(&mut self, new: u64) {
        if new > self.next_expected {
            self.next_expected = new;
        }
    }

    /// Idempotent ingestion: ensures we don’t fetch/push already handled blocks
    pub async fn ingest_block(
        ingestion: Arc<Mutex<Self>>,
        eth_client: Arc<EthClient>,
        block_queue: Arc<AsyncUniqueHeap<OrderedBlock, u64>>,
        signed_block_number: u64,
    ) -> Result<(), RpcErr> {
        // lock only around state check/update
        {
            let mut state = ingestion.lock().await;

            // skip if already handled
            if signed_block_number < state.next_expected {
                tracing::debug!(
                    "Skipping block {signed_block_number}, already past next_expected={}",
                    state.next_expected
                );
                return Ok(());
            }

            // reserve next expected here, so concurrent calls won’t re-fetch
            state.advance(signed_block_number + 1);
        }

        // do the actual work outside of lock
        let block = eth_client
            .get_block_by_number(BlockIdentifier::Number(signed_block_number))
            .await
            .map_err(|error| RpcErr::Internal(error.to_string()))?;

        let block = rpc_block_to_block(block);
        block_queue.push(OrderedBlock(block)).await;

        tracing::info!("Ingested block {signed_block_number}");

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
