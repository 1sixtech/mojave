use tokio_util::sync::CancellationToken;
use zeromq::{Socket, SubSocket};

use crate::block::{BlockWatcher, BlockWatcherHandle, Result};

pub struct BlockWatcherBuilder {
    socket_url: String,
    max_channel_capacity: usize,
    subscription_topic: String,
    shutdown: CancellationToken,
}

impl BlockWatcherBuilder {
    pub fn new(socket_url: &str, shutdown: CancellationToken) -> Self {
        const SUBSCRIBE_TOPIC: &str = "rawblock";
        const MAX_CHANNEL_CAPACITY: usize = 1000;

        Self {
            socket_url: socket_url.to_string(),
            max_channel_capacity: MAX_CHANNEL_CAPACITY,
            subscription_topic: SUBSCRIBE_TOPIC.to_string(),
            shutdown,
        }
    }

    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.max_channel_capacity = capacity;
        self
    }

    pub fn with_topic(mut self, topic: &str) -> Self {
        self.subscription_topic = topic.to_string();
        self
    }

    pub async fn spawn(self) -> Result<BlockWatcherHandle> {
        let mut socket = SubSocket::new();
        socket.connect(&self.socket_url).await?;
        socket.subscribe(&self.subscription_topic).await?;

        let (sender, _) = tokio::sync::broadcast::channel(self.max_channel_capacity);

        let mut worker = BlockWatcher {
            socket,
            shutdown: self.shutdown.clone(),
            sender: sender.clone(),
        };

        let join = tokio::spawn(async move { worker.watch().await });

        Ok(BlockWatcherHandle {
            sender,
            shutdown: self.shutdown,
            join,
        })
    }
}
