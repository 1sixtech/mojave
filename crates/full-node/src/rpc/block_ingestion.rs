use ethrex_common::types::{Block, BlockBody, Transaction};
use ethrex_rpc::{
    RpcErr,
    types::{block::RpcBlock, block_identifier::BlockIdentifier},
};

use crate::rpc::{RpcApiContext, types::OrderedBlock};

use tokio::sync::{
    mpsc::{self, error::TrySendError},
    oneshot,
};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};

#[derive(Debug, Clone)]
pub struct BlockIngestion {
    sender: mpsc::Sender<Message>,
    current_block_number: u64,
}

impl BlockIngestion {
    pub fn start(context: RpcApiContext, channel_capacity: usize, start_block_number: u64) -> Self {
        let (sender, receiver) = mpsc::channel(channel_capacity);
        let mut receiver = ReceiverStream::new(receiver);

        tokio::spawn(async move {
            while let Some(message) = receiver.next().await {
                handle_message(&context, message).await;
            }
        });
        Self {
            sender,
            current_block_number: start_block_number,
        }
    }

    pub async fn ingest_block(&self) -> Result<(), RpcErr> {
        let (sender, receiver) = oneshot::channel();
        let block_number = self.current_block_number;

        self.sender
            .try_send(Message::IngestBlock(sender, block_number))
            .map_err(|error| match error {
                TrySendError::Full(_) => {
                    RpcErr::Internal("Block ingestion channel full".to_string())
                }
                TrySendError::Closed(_) => {
                    RpcErr::Internal("Block ingestion channel closed".to_string())
                }
            })?;
        receiver
            .await
            .map_err(|_| RpcErr::Internal("Failed to receive block ingestion result".to_string()))
            .map_err(|e| RpcErr::Internal(e.to_string()))
            .map(|_| ())
    }

    pub fn advance_block_number(&mut self, amount: u64) {
        self.current_block_number += amount;
    }

    pub fn get_current_block_number(&self) -> u64 {
        self.current_block_number
    }
}

async fn handle_message(context: &RpcApiContext, message: Message) {
    match message {
        Message::IngestBlock(sender, block_number) => {
            let _ = sender.send(context.block_ingestion(block_number).await);
        }
    }
}

enum Message {
    IngestBlock(oneshot::Sender<Result<(), RpcErr>>, u64),
}

impl RpcApiContext {
    pub(crate) async fn block_ingestion(&self, block_number: u64) -> Result<(), RpcErr> {
        if self.pending_signed_blocks.len().await == 0 {
            return Err(RpcErr::Internal(
                "No pending signed blocks, no ingestion needed".to_string(),
            ));
        }

        let peek = self.pending_signed_blocks.peek().await;

        // peek must be not none now
        if peek.is_none() {
            return Err(RpcErr::Internal(
                "No pending signed blocks, no ingestion needed".to_string(),
            ));
        }

        if block_number != peek.unwrap().0.header.number {
            let rpc_block = self
                .eth_client
                .get_block_by_number(BlockIdentifier::Number(block_number))
                .await
                .map_err(|e| RpcErr::Internal(e.to_string()))?;

            let block = rpc_block_to_block(rpc_block);

            self.block_queue.push(OrderedBlock(block)).await;
        } else {
            let signed_block = self.pending_signed_blocks.pop().await.unwrap();

            self.block_queue.push(signed_block).await;
        }

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
