use bitcoin::Block;
use tokio::{sync::broadcast::Sender, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::block::{BlockWatcher, Result};

pub struct BlockWatcherHandle {
    pub(crate) sender: Sender<Block>,
    pub(crate) shutdown: CancellationToken,
    pub(crate) join: JoinHandle<Result<()>>,
}

impl BlockWatcherHandle {
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Block> {
        self.sender.subscribe()
    }

    pub fn shutdown(&self) {
        self.shutdown.cancel();
    }

    pub async fn join(self) -> Result<()> {
        self.join.await?
    }

    /// Utility function to spawn a new block watcher with a single receiver.
    /// Avoid the need for manual spawning and managing the receiver.
    pub async fn spawn_with_receiver(
        socket_url: &str,
        shutdown: CancellationToken,
        max_channel_capacity: usize,
    ) -> Result<(Self, tokio::sync::broadcast::Receiver<Block>)> {
        let handle = BlockWatcher::spawn(socket_url, shutdown, max_channel_capacity).await?;
        let receiver = handle.subscribe();
        Ok((handle, receiver))
    }
}
