use bitcoin::consensus::deserialize;
use tokio_util::sync::CancellationToken;
use zeromq::{Socket, SocketRecv, SubSocket, ZmqMessage};

use crate::{error::Result, watch::WatcherHandle};

/// ZMQ message format: frame 0 is topic, frame 1 is payload.
/// We expect at least 2 frames: [topic, payload].
const ZMQ_MESSAGE_MIN_FRAMES: usize = 2;
const ZMQ_PAYLOAD_FRAME_INDEX: usize = 1;

/// Trait describing the default subscription topic for a watcher type.
pub trait Topic {
    /// ZMQ topic to subscribe to.
    const TOPIC: &'static str;
}

/// Generic ZMQ watcher.
pub struct Watcher<T> {
    pub(crate) socket: SubSocket,
    pub(crate) shutdown: CancellationToken,
    pub(crate) sender: tokio::sync::broadcast::Sender<T>,
}

impl<T> Watcher<T>
where
    T: Topic + bitcoin::consensus::Decodable + Send + Clone + 'static + core::fmt::Debug,
{
    pub async fn spawn(
        socket_url: &str,
        shutdown: CancellationToken,
        max_channel_capacity: usize,
    ) -> Result<WatcherHandle<T>, T> {
        let mut socket = SubSocket::new();
        socket.connect(socket_url).await?;
        socket.subscribe(T::TOPIC).await?;

        let (sender, _) = tokio::sync::broadcast::channel(max_channel_capacity);

        let mut worker = Watcher {
            socket,
            shutdown: shutdown.clone(),
            sender: sender.clone(),
        };

        let join = tokio::spawn(async move { worker.watch().await });

        Ok(WatcherHandle {
            sender,
            shutdown,
            join,
        })
    }

    pub(crate) async fn watch(&mut self) -> Result<(), T> {
        tracing::info!("Watcher started");

        loop {
            tokio::select! {
                biased;

                _ = self.shutdown.cancelled() => {
                    tracing::info!("Watcher shutting down gracefully");
                    return Ok(());
                }

                msg = self.socket.recv() => self.process_message(msg?).await?,
            }
        }
    }

    #[inline]
    async fn process_message(&self, msg: ZmqMessage) -> Result<(), T> {
        if msg.len() < ZMQ_MESSAGE_MIN_FRAMES {
            tracing::debug!("ZMQ message without payload; skipping");
            return Ok(());
        }

        let Some(payload) = &msg.get(ZMQ_PAYLOAD_FRAME_INDEX) else {
            tracing::warn!("Unable to get payload");
            return Ok(());
        };

        let item = deserialize::<T>(payload)?;
        tracing::debug!("Received item");

        self.sender.send(item)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::sequence::Sequence;

    use super::*;
    use bitcoin::{Block, Transaction};

    use zeromq::Socket;

    #[test]
    fn test_topic_trait_implementations() {
        assert_eq!(Block::TOPIC, "rawblock");
        assert_eq!(Transaction::TOPIC, "rawtx");
        assert_eq!(Sequence::TOPIC, "sequence");
    }

    #[tokio::test]
    async fn test_watcher_spawn_fails_with_invalid_url() {
        let shutdown = CancellationToken::new();

        let result = Watcher::<Block>::spawn("invalid://url", shutdown, 100).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_watcher_spawn_fails_with_invalid_protocol() {
        let shutdown = CancellationToken::new();

        let result = Watcher::<Block>::spawn("http://localhost:28332", shutdown, 100).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_watcher_creation_direct() {
        let shutdown = CancellationToken::new();
        let (sender, _) = tokio::sync::broadcast::channel(100);
        let socket = SubSocket::new();

        let watcher = Watcher::<Block> {
            socket,
            shutdown: shutdown.clone(),
            sender,
        };

        assert!(!watcher.shutdown.is_cancelled());
        assert_eq!(watcher.sender.receiver_count(), 0);
    }

    #[test]
    fn test_watcher_with_different_shutdown_tokens() {
        let shutdown1 = CancellationToken::new();
        let shutdown2 = CancellationToken::new();
        let (sender, _) = tokio::sync::broadcast::channel(100);
        let socket = SubSocket::new();

        let watcher = Watcher::<Block> {
            socket,
            shutdown: shutdown1.clone(),
            sender,
        };

        assert!(!watcher.shutdown.is_cancelled());
        assert!(!shutdown1.is_cancelled());
        assert!(!shutdown2.is_cancelled());

        shutdown1.cancel();
        assert!(watcher.shutdown.is_cancelled());
        assert!(shutdown1.is_cancelled());
        assert!(!shutdown2.is_cancelled());
    }

    #[test]
    fn test_watcher_sender_properties() {
        let shutdown = CancellationToken::new();
        let (sender, _) = tokio::sync::broadcast::channel(50);
        let socket = SubSocket::new();

        let watcher = Watcher::<Transaction> {
            socket,
            shutdown,
            sender,
        };

        assert_eq!(watcher.sender.receiver_count(), 0);

        let _receiver1 = watcher.sender.subscribe();
        assert_eq!(watcher.sender.receiver_count(), 1);

        let _receiver2 = watcher.sender.subscribe();
        assert_eq!(watcher.sender.receiver_count(), 2);
    }
}
