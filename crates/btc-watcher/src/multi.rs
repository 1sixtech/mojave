use bitcoin::{Block, Transaction, consensus::deserialize};
use tokio_util::sync::CancellationToken;

use crate::{
    error::{Error, Result},
    sequence::Sequence,
    types::MultiWatcherBuilder,
    watch::{Decodable, Topics},
};

#[derive(Debug, Clone)]
pub enum Multi {
    Block(Block),
    Transaction(Transaction),
    Sequence(Sequence),
}

impl Topics for Multi {
    const TOPICS: &'static [&'static str] = &["rawblock", "rawtx", "sequence"];
}

impl Decodable for Multi {
    #[inline]
    fn decode(topic: &str, payload: &[u8]) -> Result<Self, Self> {
        match topic {
            "rawblock" => {
                let block: Block = deserialize(payload).map_err(Error::DeserializationError)?;
                Ok(Multi::Block(block))
            }
            "rawtx" => {
                let tx: Transaction = deserialize(payload).map_err(Error::DeserializationError)?;
                Ok(Multi::Transaction(tx))
            }
            "sequence" => {
                let seq: Sequence = deserialize(payload).map_err(Error::DeserializationError)?;
                Ok(Multi::Sequence(seq))
            }
            _ => Err(Error::DeserializationError(
                bitcoin::consensus::encode::Error::ParseFailed("Unknown topic"),
            )),
        }
    }
}

/// Helper to create a builder with default configuration.
pub fn builder(socket_url: &str, shutdown: CancellationToken) -> MultiWatcherBuilder {
    MultiWatcherBuilder::new(socket_url, shutdown)
}
