use bitcoin::{Transaction, consensus::deserialize};
use tokio_util::sync::CancellationToken;

use crate::{
    error::Error,
    types::TransactionWatcherBuilder,
    watch::{Decodable, Topics},
};

impl Topics for Transaction {
    const TOPICS: &'static [&'static str] = &["rawtx"];
}

impl Decodable for Transaction {
    fn decode(_topic: &str, payload: &[u8]) -> core::result::Result<Self, Error<Self>> {
        deserialize(payload).map_err(Error::DeserializationError)
    }
}

/// Helper to create a builder with default configuration.
pub fn builder(socket_url: &str, shutdown: CancellationToken) -> TransactionWatcherBuilder {
    TransactionWatcherBuilder::new(socket_url, shutdown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{Amount, OutPoint, TxIn, TxOut, consensus::encode::serialize_hex};
    use mojave_tests::assert_type;

    fn create_test_transaction() -> Transaction {
        Transaction {
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
        }
    }

    #[test]
    fn test_transaction_topic() {
        assert_eq!(Transaction::TOPICS, ["rawtx"]);
    }

    #[test]
    fn test_builder_creates_transaction_watcher_builder() {
        let shutdown = CancellationToken::new();
        let builder = builder("tcp://localhost:28332", shutdown.clone());

        // Test that the builder is the correct type
        assert_type::<TransactionWatcherBuilder>(builder);
    }

    #[tokio::test]
    async fn test_transaction_watcher_builder_spawn_fails_with_invalid_url() {
        let shutdown = CancellationToken::new();
        let builder = builder("invalid://url", shutdown);

        let result = builder.spawn().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_serialization_compatibility() {
        let tx = create_test_transaction();
        let serialized = serialize_hex(&tx);

        // Should be able to serialize without panicking
        assert!(!serialized.is_empty());
    }
}
