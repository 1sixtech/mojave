use bitcoin::{Block, consensus::deserialize};
use tokio_util::sync::CancellationToken;
use zeromq::{Socket, SocketRecv, SubSocket, ZmqMessage};

use crate::block::{BlockWatcherHandle, Result};

pub struct BlockWatcher {
    pub(crate) socket: SubSocket,
    pub(crate) shutdown: CancellationToken,
    pub(crate) sender: tokio::sync::broadcast::Sender<Block>,
}

impl BlockWatcher {
    pub async fn spawn(
        socket_url: &str,
        shutdown: CancellationToken,
        max_channel_capacity: usize,
    ) -> Result<BlockWatcherHandle> {
        const SUBSCRIPTION_TOPIC: &str = "rawblock";

        let mut socket = SubSocket::new();
        socket.connect(socket_url).await?;
        socket.subscribe(SUBSCRIPTION_TOPIC).await?;

        let (sender, _) = tokio::sync::broadcast::channel(max_channel_capacity);

        let mut worker = BlockWatcher {
            socket,
            shutdown: shutdown.clone(),
            sender: sender.clone(),
        };

        let join = tokio::spawn(async move { worker.watch().await });

        Ok(BlockWatcherHandle {
            sender,
            shutdown,
            join,
        })
    }

    pub(crate) async fn watch(&mut self) -> Result<()> {
        tracing::info!("BlockWatcher started");

        loop {
            tokio::select! {
                biased;

                _ = self.shutdown.cancelled() => {
                    tracing::info!("BlockWatcher shutting down gracefully");
                    return Ok(());
                }

                msg = self.socket.recv() => self.process_message(msg?).await?,
            }
        }
    }

    #[inline]
    async fn process_message(&self, msg: ZmqMessage) -> Result<()> {
        if msg.len() < 2 {
            tracing::debug!("ZMQ message without payload; skipping");
            return Ok(());
        }

        let Some(payload) = &msg.get(1) else {
            tracing::warn!("Unable to get payload");
            return Ok(());
        };

        let block = deserialize::<Block>(payload)?;
        tracing::debug!(
            "Received block: hash={}, height={}",
            block.block_hash(),
            block.bip34_block_height().unwrap_or(0)
        );

        self.sender.send(block)?;

        Ok(())
    }
}
