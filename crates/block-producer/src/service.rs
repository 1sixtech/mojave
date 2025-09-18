use crate::{
    BlockProducerContext,
    error::{Error, Result},
    types::BlockProducerOptions,
};
use ethrex_common::types::Block;
use mojave_node_lib::types::MojaveNode;
use std::time::Duration;
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    oneshot,
};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tracing::error;

pub async fn run(
    node: MojaveNode,
    block_producer_options: &BlockProducerOptions,
) -> Result<()> {
    let context = BlockProducerContext::new(
        node.store.clone(),
        node.blockchain.clone(),
        node.rollup_store.clone(),
        node.genesis.coinbase,
    );
    let block_time = block_producer_options.block_time;
    let block_producer = BlockProducer::start(context, 100);

    let cancel_token = node.cancel_token.clone();

    tokio::select! {
        _ = async {
            loop {
                if let Err(error) = block_producer.build_block().await {
                    tracing::error!("Failed to build a block: {}", error);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(block_time)).await;
            }
        } => {}
        _ = cancel_token.cancelled() => {
            tracing::info!("Shutting down block producer");
        }
    }


    Ok(())
}

#[derive(Clone)]
pub struct BlockProducer {
    sender: mpsc::Sender<Message>,
}

impl BlockProducer {
    pub fn start(context: BlockProducerContext, channel_capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(channel_capacity);
        let mut receiver = ReceiverStream::new(receiver);

        tokio::spawn(async move {
            while let Some(message) = receiver.next().await {
                handle_message(&context, message).await;
            }

            error!("Block builder stopped because the sender dropped.");
        });
        Self { sender }
    }

    pub async fn build_block(&self) -> Result<Block> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .try_send(Message::BuildBlock(sender))
            .map_err(|error| match error {
                TrySendError::Full(_) => Error::Full,
                TrySendError::Closed(_) => Error::Stopped,
            })?;
        receiver.await?
    }
}

async fn handle_message(context: &BlockProducerContext, message: Message) {
    match message {
        Message::BuildBlock(sender) => {
            if let Err(e) = sender.send(context.build_block().await) {
                tracing::warn!(error = ?e, "Failed to send built block over channel");
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum Message {
    BuildBlock(oneshot::Sender<Result<Block>>),
}
