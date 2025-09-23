use crate::{BlockProducerContext, error::Result, types::BlockProducerOptions};
use ethrex_common::types::Block;
use mojave_node_lib::types::MojaveNode;
use mojave_task::Task;
use std::time::Duration;

pub async fn run(node: MojaveNode, block_producer_options: &BlockProducerOptions) -> Result<()> {
    let context = BlockProducerContext::new(
        node.store.clone(),
        node.blockchain.clone(),
        node.rollup_store.clone(),
        node.genesis.coinbase,
    );
    let block_time = block_producer_options.block_time;
    let handle = context.spawn_with_capacity(100);

    let block_producer_for_loop = handle.clone();
    tokio::spawn(async move {
        loop {
            match block_producer_for_loop.request(Request::BuildBlock).await {
                Ok(Response::Block(block)) => {
                    tracing::info!("Block built: {}", block.header.number)
                }
                Err(error) => {
                    tracing::error!("Failed to build a block: {}", error);
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(block_time)).await;
        }
    });

    mojave_utils::signal::wait_for_shutdown_signal().await?;
    if let Err(error) = handle.shutdown().await {
        tracing::warn!(error = ?error, "Failed to shutdown block producer");
    }
    Ok(())
}

impl mojave_task::Task for BlockProducerContext {
    type Request = Request;
    type Response = Response;
    type Error = crate::error::Error;

    async fn handle_request(&self, request: Request) -> Result<Self::Response> {
        match request {
            Request::BuildBlock => Ok(Response::Block(self.build_block().await?)),
        }
    }

    async fn on_shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down block producer");
        Ok(())
    }
}

pub enum Request {
    BuildBlock,
}

#[derive(Debug)]
pub enum Response {
    Block(Block),
}
