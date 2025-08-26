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
        let builder = builder("tcp://localhost:28332", shutdown.clone());

        // Test that the builder is the correct type
        let _: BlockWatcherBuilder = builder;
    }

    #[tokio::test]
    async fn test_block_watcher_builder_spawn_fails_with_invalid_url() {
        let shutdown = CancellationToken::new();
        let builder = builder("invalid://url", shutdown);

        let result = builder.spawn().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_block_clone_debug_traits() {
        let block = create_test_block();

        // Test Clone trait
        let cloned_block = block.clone();
        assert_eq!(block.header.version, cloned_block.header.version);
        assert_eq!(
            block.header.prev_blockhash,
            cloned_block.header.prev_blockhash
        );
        assert_eq!(block.header.merkle_root, cloned_block.header.merkle_root);
        assert_eq!(block.txdata.len(), cloned_block.txdata.len());

        // Test Debug trait
        let debug_str = format!("{block:?}");
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("Block"));
    }

    #[test]
    fn test_block_header_properties() {
        let block = create_test_block();

        // Test block header properties
        assert_eq!(block.header.version, bitcoin::block::Version::ONE);
        assert_eq!(block.header.prev_blockhash, BlockHash::all_zeros());
        assert_eq!(block.header.merkle_root, TxMerkleNode::all_zeros());
        assert_eq!(block.header.time, 1234567890);
        assert_eq!(block.header.nonce, 2083236893);
    }

    #[test]
    fn test_block_with_transactions() {
        use bitcoin::{Amount, OutPoint, Transaction, TxIn, TxOut};

        let mut block = create_test_block();
        block.txdata.push(Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: bitcoin::ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![TxOut {
                value: Amount::from_sat(50_000_000),
                script_pubkey: bitcoin::ScriptBuf::new(),
            }],
        });

        assert_eq!(block.txdata.len(), 1);

        // Test that it's still cloneable and debuggable
        let cloned_block = block.clone();
        assert_eq!(cloned_block.txdata.len(), 1);

        let debug_str = format!("{block:?}");
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_block_hash_calculation() {
        let block = create_test_block();
        let block_hash = block.block_hash();

        // Should be able to calculate hash without panicking
        assert_ne!(block_hash, BlockHash::all_zeros());

        // Hash should be consistent
        let block_hash2 = block.block_hash();
        assert_eq!(block_hash, block_hash2);
    }
}
