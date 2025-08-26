use bitcoin::{Block, consensus::deserialize};
use tokio_util::sync::CancellationToken;

use crate::{
    error::Error,
    types::BlockWatcherBuilder,
    watch::{Decodable, Topics},
};

impl Topics for Block {
    const TOPICS: &'static [&'static str] = &["rawblock"];
}

impl Decodable for Block {
    #[inline]
    fn decode(_topic: &str, payload: &[u8]) -> core::result::Result<Self, Error<Self>> {
        deserialize(payload).map_err(Error::DeserializationError)
    }
}

pub type Result<T> = core::result::Result<T, Error<Block>>;

/// Helper to create a builder with default configuration.
pub fn builder(socket_url: &str, shutdown: CancellationToken) -> BlockWatcherBuilder {
    BlockWatcherBuilder::new(socket_url, shutdown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Block;
    use mojave_tests::assert_type;

    #[test]
    fn test_block_topic() {
        assert_eq!(Block::TOPICS, &["rawblock"]);
    }

    #[test]
    fn test_builder_creates_block_watcher_builder() {
        let shutdown = CancellationToken::new();
        let builder = builder("tcp://localhost:28332", shutdown);

        // Test that the builder is the correct type
        assert_type::<BlockWatcherBuilder>(builder);
    }

    #[tokio::test]
    async fn test_block_watcher_builder_spawn_fails_with_invalid_url() {
        let shutdown = CancellationToken::new();
        let builder = builder("invalid://url", shutdown);

        let result = builder.spawn().await;
        assert!(result.is_err());
    }
}
