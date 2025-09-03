use tokio_util::sync::CancellationToken;
use zeromq::{Socket, SubSocket};

use crate::{
    error::Result,
    watch::{Decodable, Topics, Watcher, WatcherHandle},
};

/// Builder used for configuring and spawning watchers.
pub struct WatcherBuilder<T> {
    socket_url: String,
    max_channel_capacity: usize,
    subscription_topics: Vec<String>,
    shutdown: CancellationToken,
    _marker: core::marker::PhantomData<T>,
}

impl<T> WatcherBuilder<T>
where
    T: Topics + Decodable + Send + Clone + 'static + core::fmt::Debug,
{
    pub fn new(socket_url: &str, shutdown: CancellationToken) -> Self {
        const MAX_CHANNEL_CAPACITY: usize = 1000;

        Self {
            socket_url: socket_url.to_string(),
            max_channel_capacity: MAX_CHANNEL_CAPACITY,
            subscription_topics: T::TOPICS.iter().map(|s| s.to_string()).collect(),
            shutdown,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.max_channel_capacity = capacity;
        self
    }

    pub fn with_topic(mut self, topic: &str) -> Self {
        self.subscription_topics = vec![topic.to_string()];
        self
    }

    pub fn with_topics<I, S>(mut self, topics: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.subscription_topics = topics.into_iter().map(|s| s.as_ref().to_string()).collect();
        self
    }

    pub async fn spawn(self) -> Result<WatcherHandle<T>, T> {
        let mut socket = SubSocket::new();
        socket.connect(&self.socket_url).await?;
        for topic in &self.subscription_topics {
            socket.subscribe(topic).await?;
        }

        let (sender, _) = tokio::sync::broadcast::channel(self.max_channel_capacity);

        let mut worker = Watcher {
            socket,
            shutdown: self.shutdown.clone(),
            sender: sender.clone(),
        };

        let join = tokio::spawn(async move { worker.watch().await });

        Ok(WatcherHandle {
            sender,
            shutdown: self.shutdown,
            join,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::sequence::Sequence;

    use super::*;
    use bitcoin::{Block, Transaction};

    #[test]
    fn test_watcher_builder_new_sets_defaults() {
        let shutdown = CancellationToken::new();
        let builder = WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown.clone());

        assert_eq!(builder.socket_url, "tcp://localhost:28332");
        assert_eq!(builder.max_channel_capacity, 1000);
        assert_eq!(builder.subscription_topics, Block::TOPICS);
        assert!(!builder.shutdown.is_cancelled());
        assert!(!shutdown.is_cancelled());
    }

    #[test]
    fn test_watcher_builder_new_transaction_type() {
        let shutdown = CancellationToken::new();
        let builder = WatcherBuilder::<Transaction>::new("tcp://localhost:28332", shutdown);

        assert_eq!(builder.socket_url, "tcp://localhost:28332");
        assert_eq!(builder.subscription_topics, Transaction::TOPICS);
        assert_eq!(builder.subscription_topics, vec!["rawtx"]);
    }

    #[test]
    fn test_watcher_builder_new_sequence_type() {
        let shutdown = CancellationToken::new();
        let builder = WatcherBuilder::<Sequence>::new("tcp://localhost:28332", shutdown);

        assert_eq!(builder.socket_url, "tcp://localhost:28332");
        assert_eq!(builder.subscription_topics, Sequence::TOPICS);
        assert_eq!(builder.subscription_topics, vec!["sequence"]);
    }

    #[test]
    fn test_with_capacity_sets_capacity() {
        let shutdown = CancellationToken::new();
        let builder =
            WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown).with_capacity(500);

        assert_eq!(builder.max_channel_capacity, 500);
    }

    #[test]
    fn test_with_topic_sets_topic() {
        let shutdown = CancellationToken::new();
        let builder =
            WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown).with_topic("hashtx");

        assert_eq!(builder.subscription_topics, vec!["hashtx"]);
    }

    #[test]
    fn test_builder_chaining() {
        let shutdown = CancellationToken::new();
        let builder = WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown)
            .with_capacity(200)
            .with_topic("rawtx");

        assert_eq!(builder.max_channel_capacity, 200);
        assert_eq!(builder.subscription_topics, vec!["rawtx"]);
    }

    #[tokio::test]
    async fn test_spawn_fails_with_invalid_url() {
        let shutdown = CancellationToken::new();
        let builder = WatcherBuilder::<Block>::new("invalid://url", shutdown);

        let result = builder.spawn().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_spawn_fails_with_invalid_protocol() {
        let shutdown = CancellationToken::new();
        let builder = WatcherBuilder::<Transaction>::new("http://localhost:28332", shutdown);

        let result = builder.spawn().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_with_zero_capacity() {
        let shutdown = CancellationToken::new();
        let builder =
            WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown).with_capacity(0);

        assert_eq!(builder.max_channel_capacity, 0);
    }

    #[test]
    fn test_builder_with_empty_topic() {
        let shutdown = CancellationToken::new();
        let builder =
            WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown).with_topic("");

        assert_eq!(builder.subscription_topics, vec![""]);
    }

    #[test]
    fn test_builder_with_custom_url() {
        let shutdown = CancellationToken::new();
        let custom_url = "tcp://127.0.0.1:18332";
        let builder = WatcherBuilder::<Block>::new(custom_url, shutdown);

        assert_eq!(builder.socket_url, custom_url);
    }

    #[test]
    fn test_builder_with_ipc_url() {
        let shutdown = CancellationToken::new();
        let ipc_url = "ipc:///tmp/bitcoin.sock";
        let builder = WatcherBuilder::<Transaction>::new(ipc_url, shutdown);

        assert_eq!(builder.socket_url, ipc_url);
    }

    #[test]
    fn test_builder_with_cancelled_shutdown_token() {
        let shutdown = CancellationToken::new();
        shutdown.cancel();

        let builder = WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown.clone());

        assert!(builder.shutdown.is_cancelled());
        assert!(shutdown.is_cancelled());
    }

    #[test]
    fn test_different_type_builders_have_different_topics() {
        let shutdown = CancellationToken::new();

        let block_builder = WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown.clone());
        let tx_builder =
            WatcherBuilder::<Transaction>::new("tcp://localhost:28332", shutdown.clone());
        let seq_builder = WatcherBuilder::<Sequence>::new("tcp://localhost:28332", shutdown);

        assert_eq!(block_builder.subscription_topics, vec!["rawblock"]);
        assert_eq!(tx_builder.subscription_topics, vec!["rawtx"]);
        assert_eq!(seq_builder.subscription_topics, vec!["sequence"]);
    }

    #[test]
    fn test_builder_method_chaining_order() {
        let shutdown = CancellationToken::new();

        let builder1 = WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown.clone())
            .with_capacity(100)
            .with_topic("test1");

        let builder2 = WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown)
            .with_topic("test2")
            .with_capacity(200);

        assert_eq!(builder1.max_channel_capacity, 100);
        assert_eq!(builder1.subscription_topics, vec!["test1"]);
        assert_eq!(builder2.max_channel_capacity, 200);
        assert_eq!(builder2.subscription_topics, vec!["test2"]);
    }

    #[test]
    fn test_builder_topic_override() {
        let shutdown = CancellationToken::new();
        let builder = WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown)
            .with_topic("custom_topic");

        assert_ne!(builder.subscription_topics, Block::TOPICS);
        assert_eq!(builder.subscription_topics, vec!["custom_topic"]);
    }

    #[test]
    fn test_builder_large_capacity() {
        let shutdown = CancellationToken::new();
        let large_capacity = 1_000_000;
        let builder = WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown)
            .with_capacity(large_capacity);

        assert_eq!(builder.max_channel_capacity, large_capacity);
    }

    #[test]
    fn test_builder_url_variations() {
        let shutdown = CancellationToken::new();

        let tcp_builder = WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown.clone());
        let tcp_ip_builder =
            WatcherBuilder::<Block>::new("tcp://127.0.0.1:28332", shutdown.clone());
        let ipc_builder = WatcherBuilder::<Block>::new("ipc:///tmp/test.sock", shutdown);

        assert_eq!(tcp_builder.socket_url, "tcp://localhost:28332");
        assert_eq!(tcp_ip_builder.socket_url, "tcp://127.0.0.1:28332");
        assert_eq!(ipc_builder.socket_url, "ipc:///tmp/test.sock");
    }

    #[test]
    fn test_builder_multiple_topic_overrides() {
        let shutdown = CancellationToken::new();
        let builder = WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown)
            .with_topic("topic1")
            .with_topic("topic2")
            .with_topic("final_topic");

        assert_eq!(builder.subscription_topics, vec!["final_topic"]);
    }

    #[test]
    fn test_builder_multiple_capacity_overrides() {
        let shutdown = CancellationToken::new();
        let builder = WatcherBuilder::<Block>::new("tcp://localhost:28332", shutdown)
            .with_capacity(100)
            .with_capacity(200)
            .with_capacity(300);

        assert_eq!(builder.max_channel_capacity, 300);
    }
}
