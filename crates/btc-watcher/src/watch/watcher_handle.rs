use tokio_util::sync::CancellationToken;

use crate::watch::{Result, Topic, WatcherBuilder};

/// Handle to an active watcher.
pub struct WatcherHandle<T>
where
    T: Clone + core::fmt::Debug,
{
    pub(crate) sender: tokio::sync::broadcast::Sender<T>,
    pub(crate) shutdown: CancellationToken,
    pub(crate) join: tokio::task::JoinHandle<Result<(), T>>,
}

impl<T> WatcherHandle<T>
where
    T: Topic + bitcoin::consensus::Decodable + Send + Clone + 'static + core::fmt::Debug,
{
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<T> {
        self.sender.subscribe()
    }

    pub fn shutdown(&self) {
        self.shutdown.cancel();
    }

    pub async fn join(self) -> Result<(), T> {
        self.join.await?
    }

    /// Utility function to spawn a watcher with a single receiver.
    pub async fn spawn_with_receiver(
        socket_url: &str,
        shutdown: CancellationToken,
        max_channel_capacity: usize,
    ) -> Result<(Self, tokio::sync::broadcast::Receiver<T>), T> {
        let handle = WatcherBuilder::<T>::new(socket_url, shutdown)
            .with_capacity(max_channel_capacity)
            .spawn()
            .await?;
        let receiver = handle.subscribe();
        Ok((handle, receiver))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Block;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn test_watcher_handle_creation() {
        let shutdown = CancellationToken::new();
        let (sender, _) = tokio::sync::broadcast::channel::<Block>(100);
        let join = tokio::spawn(async { Ok(()) });

        let handle = WatcherHandle {
            sender,
            shutdown: shutdown.clone(),
            join,
        };

        // Test that handle contains the shutdown token
        assert!(!handle.shutdown.is_cancelled());
    }

    #[tokio::test]
    async fn test_subscribe_creates_receiver() {
        let shutdown = CancellationToken::new();
        let (sender, _) = tokio::sync::broadcast::channel::<Block>(100);
        let join = tokio::spawn(async { Ok(()) });

        let handle = WatcherHandle {
            sender,
            shutdown,
            join,
        };

        let receiver = handle.subscribe();

        // Receiver should be properly configured
        assert_eq!(receiver.len(), 0);
    }

    #[tokio::test]
    async fn test_shutdown_cancels_token() {
        let shutdown = CancellationToken::new();
        let (sender, _) = tokio::sync::broadcast::channel::<Block>(100);
        let join = tokio::spawn(async { Ok(()) });

        let handle = WatcherHandle {
            sender,
            shutdown: shutdown.clone(),
            join,
        };

        assert!(!shutdown.is_cancelled());
        handle.shutdown();
        assert!(shutdown.is_cancelled());
    }

    #[tokio::test]
    async fn test_join_waits_for_task_completion() {
        let shutdown = CancellationToken::new();
        let (sender, _) = tokio::sync::broadcast::channel::<Block>(100);

        // Create a task that completes successfully
        let join = tokio::spawn(async { Ok(()) });

        let handle = WatcherHandle {
            sender,
            shutdown,
            join,
        };

        let result = handle.join().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_join_propagates_task_error() {
        let shutdown = CancellationToken::new();
        let (sender, _) = tokio::sync::broadcast::channel::<Block>(100);

        // Create a task that panics
        let join = tokio::spawn(async { panic!("test panic") });

        let handle = WatcherHandle {
            sender,
            shutdown,
            join,
        };

        let result = handle.join().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let shutdown = CancellationToken::new();
        let (sender, _) = tokio::sync::broadcast::channel::<Block>(100);
        let join = tokio::spawn(async { Ok(()) });

        let handle = WatcherHandle {
            sender,
            shutdown,
            join,
        };

        let receiver1 = handle.subscribe();
        let receiver2 = handle.subscribe();
        let receiver3 = handle.subscribe();

        // All receivers should be independent
        assert_eq!(receiver1.len(), 0);
        assert_eq!(receiver2.len(), 0);
        assert_eq!(receiver3.len(), 0);
    }

    #[tokio::test]
    async fn test_spawn_with_receiver_fails_invalid_url() {
        let shutdown = CancellationToken::new();

        let result =
            WatcherHandle::<Block>::spawn_with_receiver("invalid://url", shutdown, 100).await;

        // Should fail due to invalid URL
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_with_cancelled_shutdown() {
        let shutdown = CancellationToken::new();
        shutdown.cancel();

        let (sender, _) = tokio::sync::broadcast::channel::<Block>(100);
        let join = tokio::spawn(async { Ok(()) });

        let handle = WatcherHandle {
            sender,
            shutdown: shutdown.clone(),
            join,
        };

        // Should already be cancelled
        assert!(handle.shutdown.is_cancelled());
    }

    #[tokio::test]
    async fn test_handle_shutdown_affects_child_tokens() {
        let parent_shutdown = CancellationToken::new();
        let child_shutdown = parent_shutdown.child_token();

        let (sender, _) = tokio::sync::broadcast::channel::<Block>(100);
        let join = tokio::spawn(async { Ok(()) });

        let handle = WatcherHandle {
            sender,
            shutdown: parent_shutdown.clone(),
            join,
        };

        assert!(!child_shutdown.is_cancelled());
        handle.shutdown();
        assert!(child_shutdown.is_cancelled());
    }

    #[tokio::test]
    async fn test_receiver_receives_messages() {
        use bitcoin::{
            Block, BlockHash, CompactTarget, TxMerkleNode, block::Header as BlockHeader,
            hashes::Hash,
        };

        let shutdown = CancellationToken::new();
        let (sender, _) = tokio::sync::broadcast::channel::<Block>(100);
        let join = tokio::spawn(async { Ok(()) });

        let handle = WatcherHandle {
            sender: sender.clone(),
            shutdown,
            join,
        };

        let mut receiver = handle.subscribe();

        // Send a test block
        let test_block = Block {
            header: BlockHeader {
                version: bitcoin::block::Version::ONE,
                prev_blockhash: BlockHash::all_zeros(),
                merkle_root: TxMerkleNode::all_zeros(),
                time: 1234567890,
                bits: CompactTarget::from_consensus(0x1d00ffff),
                nonce: 2083236893,
            },
            txdata: vec![],
        };

        sender.send(test_block.clone()).unwrap();

        // Receiver should get the message
        let received = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(received.is_ok());
        let received_block = received.unwrap().unwrap();
        assert_eq!(received_block.header.nonce, test_block.header.nonce);
    }
}
