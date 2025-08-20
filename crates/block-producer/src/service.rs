use crate::block_producer::{BlockProducerContext, BlockProducerError};
use ethrex_common::types::Block;
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    oneshot,
};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tracing::error;

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

    pub async fn build_block(&self) -> Result<Block, BlockProducerError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .try_send(Message::BuildBlock(sender))
            .map_err(|error| match error {
                TrySendError::Full(_) => BlockProducerError::Full,
                TrySendError::Closed(_) => BlockProducerError::Stopped,
            })?;
        receiver.await?
    }
}

async fn handle_message(context: &BlockProducerContext, message: Message) {
    match message {
        Message::BuildBlock(sender) => {
            let _ = sender.send(context.build_block().await);
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum Message {
    BuildBlock(oneshot::Sender<Result<Block, BlockProducerError>>),
}
