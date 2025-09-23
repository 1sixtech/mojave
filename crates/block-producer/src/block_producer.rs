use crate::{
    BlockProducerContext,
    error::{Result},
    types::BlockProducerOptions,
};
use std::time::Duration;
use mojave_task::Task;
use mojave_node_lib::types::MojaveNode;
use ethrex_common::types::Block;

pub async fn run(node: MojaveNode, block_producer_options: &BlockProducerOptions) -> Result<()> {
    let context = BlockProducerContext::new(
        node.store.clone(),
        node.blockchain.clone(),
        node.rollup_store.clone(),
        node.genesis.coinbase,
    );
    let block_time = block_producer_options.block_time;
    let block_producer = BlockProducer::new(context).spawn_with_capacity(100);

    let block_producer_for_loop = block_producer.clone();
    tokio::spawn(async move {
        loop {
            let response = block_producer_for_loop.request(Request::BuildBlock).await.unwrap();
            tracing::info!("Block built: {response:?}");
            tokio::time::sleep(Duration::from_millis(block_time)).await;
        }
    });

    mojave_utils::signal::wait_for_shutdown_signal().await.unwrap();
    block_producer.shutdown().await.unwrap();
    Ok(())
}

pub struct BlockProducer {
    context: BlockProducerContext, // TODO: do we need this?
}

impl BlockProducer {
    pub fn new(context: BlockProducerContext) -> Self {
        Self {
            context,
        }
    }
}


impl mojave_task::Task for BlockProducer {
    type Request = Request;
    type Response = Response;
    type Error = crate::error::Error;

    async fn handle_request(&self, request: Request) -> Result<Self::Response> {
        match request {
            Request::BuildBlock => Ok(Response::Block(self.context.build_block().await?)),
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