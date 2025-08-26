use bitcoin::Block;
use tokio_util::sync::CancellationToken;

use crate::{error::Error, types::BlockWatcherBuilder, watch::Topic};

impl Topic for Block {
    const TOPIC: &'static str = "rawblock";
}

pub type Result<T> = core::result::Result<T, Error<Block>>;

/// Helper to create a builder with default configuration.
pub fn builder(socket_url: &str, shutdown: CancellationToken) -> BlockWatcherBuilder {
    BlockWatcherBuilder::new(socket_url, shutdown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{
        Block, BlockHash, CompactTarget, TxMerkleNode, block::Header as BlockHeader, hashes::Hash,
    };
    use mojave_tests::assert_type;

    fn create_test_block() -> Block {
        Block {
            header: BlockHeader {
                version: bitcoin::block::Version::ONE,
                prev_blockhash: BlockHash::all_zeros(),
                merkle_root: TxMerkleNode::all_zeros(),
                time: 1234567890,
                bits: CompactTarget::from_consensus(0x1d00ffff),
                nonce: 2083236893,
            },
            txdata: vec![],
        }
    }

    #[test]
    fn test_block_topic() {
        assert_eq!(Block::TOPIC, "rawblock");
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
