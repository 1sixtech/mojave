use std::fmt;

use bitcoin::{
    BlockHash, Txid,
    consensus::{Decodable, encode},
    hashes::Hash,
    io::Read,
};
use tokio_util::sync::CancellationToken;

use crate::{
    error::Error,
    types::SequenceWatcherBuilder,
    watch::{Decodable as WatcherDecodable, Topics},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceEvent {
    BlockConnected,    // 'C'
    BlockDisconnected, // 'D'
    TxAdded,           // 'A'
    TxRemoved,         // 'R'
    Unknown(u8),
}

impl fmt::Display for SequenceEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SequenceEvent::BlockConnected => write!(f, "BlockConnected"),
            SequenceEvent::BlockDisconnected => write!(f, "BlockDisconnected"),
            SequenceEvent::TxAdded => write!(f, "TxAdded"),
            SequenceEvent::TxRemoved => write!(f, "TxRemoved"),
            SequenceEvent::Unknown(x) => write!(f, "Unknown(0x{x:02x})"),
        }
    }
}

/// ZMQ `-zmqpubsequence` BODY (2nd frame), decoded via `Decodable`.
///
/// Layout as sent by `bitcoind` (byte indices are 0-based, end-exclusive):
///
/// - `[0..32)`   — 32-byte object hash in **RPC/ZMQ (display) byte order**
/// - `[32]`      — 1 byte **kind**: `'C' | 'D' | 'A' | 'R'`
/// - `[33..41)`  — optional 8-byte **mempool sequence** (little-endian `u64`);
///   present **only** when kind is `'A'` or `'R'`
///
/// Therefore:
/// - Kinds `C`/`D`: body length = 33 bytes
/// - Kinds `A`/`R`: body length = 41 bytes
///
/// Reference: <https://github.com/bitcoin/bitcoin/blob/master/src/zmq/zmqpublishnotifier.cpp>
#[derive(Debug, Clone)]
pub struct Sequence {
    pub hash_bytes: [u8; 32], // raw payload bytes
    pub event: SequenceEvent,
    pub mempool_seq: Option<u64>,
}

impl fmt::Display for Sequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hash = if self.is_block() {
            self.block_hash().to_string()
        } else {
            self.txid().to_string()
        };

        match self.event {
            SequenceEvent::TxAdded | SequenceEvent::TxRemoved => {
                write!(
                    f,
                    "{} hash={} mempool_seq={}",
                    self.event,
                    hash,
                    self.mempool_seq.unwrap_or(0),
                )
            }
            _ => {
                write!(f, "{} hash={}", self.event, hash)
            }
        }
    }
}

impl Decodable for Sequence {
    fn consensus_decode_from_finite_reader<R: Read + ?Sized>(
        r: &mut R,
    ) -> core::result::Result<Self, encode::Error> {
        let mut hash_bytes = [0u8; 32];
        r.read_exact(&mut hash_bytes).map_err(encode::Error::Io)?;

        let kind: u8 = u8::consensus_decode_from_finite_reader(r)?;
        let event = match kind {
            b'C' => SequenceEvent::BlockConnected,
            b'D' => SequenceEvent::BlockDisconnected,
            b'A' => SequenceEvent::TxAdded,
            b'R' => SequenceEvent::TxRemoved,
            x => SequenceEvent::Unknown(x),
        };

        let mempool_seq = match event {
            SequenceEvent::TxAdded | SequenceEvent::TxRemoved => {
                Some(u64::consensus_decode_from_finite_reader(r)?)
            }
            _ => None,
        };

        Ok(Sequence {
            hash_bytes,
            event,
            mempool_seq,
        })
    }
}

impl Sequence {
    /// Convert payload hash to `Txid` (flips RPC/ZMQ byte order to internal).
    pub fn txid(&self) -> Txid {
        let mut b = self.hash_bytes;
        b.reverse();
        Txid::from_raw_hash(Hash::from_byte_array(b))
    }
    /// Convert payload hash to `BlockHash` (flips byte order).
    pub fn block_hash(&self) -> BlockHash {
        let mut b = self.hash_bytes;
        b.reverse();
        BlockHash::from_raw_hash(Hash::from_byte_array(b))
    }
    pub fn is_block(&self) -> bool {
        matches!(
            self.event,
            SequenceEvent::BlockConnected | SequenceEvent::BlockDisconnected
        )
    }
    pub fn is_reorg_signal(&self) -> bool {
        matches!(self.event, SequenceEvent::BlockDisconnected)
    }
}

impl Topics for Sequence {
    const TOPICS: &'static [&'static str] = &["sequence"];
}

impl WatcherDecodable for Sequence {
    #[inline]
    fn decode(_topic: &str, payload: &[u8]) -> core::result::Result<Self, Error<Self>> {
        use bitcoin::consensus::deserialize;
        deserialize(payload).map_err(Error::DeserializationError)
    }
}

/// Helper to create a builder with default configuration.
pub fn builder(socket_url: &str, shutdown: CancellationToken) -> SequenceWatcherBuilder {
    SequenceWatcherBuilder::new(socket_url, shutdown)
}

#[cfg(test)]
mod tests {
    use crate::error::Error;

    use super::*;
    use bitcoin::consensus::Encodable;
    use mojave_tests::assert_type;
    use std::io::Cursor;

    #[test]
    fn test_sequence_event_display() {
        assert_eq!(SequenceEvent::BlockConnected.to_string(), "BlockConnected");
        assert_eq!(
            SequenceEvent::BlockDisconnected.to_string(),
            "BlockDisconnected"
        );
        assert_eq!(SequenceEvent::TxAdded.to_string(), "TxAdded");
        assert_eq!(SequenceEvent::TxRemoved.to_string(), "TxRemoved");
        assert_eq!(SequenceEvent::Unknown(0x42).to_string(), "Unknown(0x42)");
    }

    #[test]
    fn test_sequence_topic() {
        assert_eq!(Sequence::TOPICS, &["sequence"]);
    }

    #[test]
    fn test_sequence_decode_block_connected() {
        let mut data = Vec::new();
        // Hash bytes (32 bytes)
        let hash_bytes = [0x01; 32];
        data.extend_from_slice(&hash_bytes);
        // Event type 'C' for BlockConnected
        data.push(b'C');

        let mut cursor = Cursor::new(data);
        let sequence = Sequence::consensus_decode_from_finite_reader(&mut cursor).unwrap();

        assert_eq!(sequence.hash_bytes, hash_bytes);
        assert_eq!(sequence.event, SequenceEvent::BlockConnected);
        assert_eq!(sequence.mempool_seq, None);
        assert!(sequence.is_block());
        assert!(!sequence.is_reorg_signal());
    }

    #[test]
    fn test_sequence_decode_block_disconnected() {
        let mut data = Vec::new();
        // Hash bytes (32 bytes)
        let hash_bytes = [0x02; 32];
        data.extend_from_slice(&hash_bytes);
        // Event type 'D' for BlockDisconnected
        data.push(b'D');

        let mut cursor = Cursor::new(data);
        let sequence = Sequence::consensus_decode_from_finite_reader(&mut cursor).unwrap();

        assert_eq!(sequence.hash_bytes, hash_bytes);
        assert_eq!(sequence.event, SequenceEvent::BlockDisconnected);
        assert_eq!(sequence.mempool_seq, None);
        assert!(sequence.is_block());
        assert!(sequence.is_reorg_signal());
    }

    #[test]
    fn test_sequence_decode_tx_added() {
        let mut data = Vec::new();
        // Hash bytes (32 bytes)
        let hash_bytes = [0x03; 32];
        data.extend_from_slice(&hash_bytes);
        // Event type 'A' for TxAdded
        data.push(b'A');
        // Mempool sequence (8 bytes, little endian)
        let mempool_seq = 12345u64;
        mempool_seq.consensus_encode(&mut data).unwrap();

        let mut cursor = Cursor::new(data);
        let sequence = Sequence::consensus_decode_from_finite_reader(&mut cursor).unwrap();

        assert_eq!(sequence.hash_bytes, hash_bytes);
        assert_eq!(sequence.event, SequenceEvent::TxAdded);
        assert_eq!(sequence.mempool_seq, Some(mempool_seq));
        assert!(!sequence.is_block());
        assert!(!sequence.is_reorg_signal());
    }

    #[test]
    fn test_sequence_decode_tx_removed() {
        let mut data = Vec::new();
        // Hash bytes (32 bytes)
        let hash_bytes = [0x04; 32];
        data.extend_from_slice(&hash_bytes);
        // Event type 'R' for TxRemoved
        data.push(b'R');
        // Mempool sequence (8 bytes, little endian)
        let mempool_seq = 54321u64;
        mempool_seq.consensus_encode(&mut data).unwrap();

        let mut cursor = Cursor::new(data);
        let sequence = Sequence::consensus_decode_from_finite_reader(&mut cursor).unwrap();

        assert_eq!(sequence.hash_bytes, hash_bytes);
        assert_eq!(sequence.event, SequenceEvent::TxRemoved);
        assert_eq!(sequence.mempool_seq, Some(mempool_seq));
        assert!(!sequence.is_block());
        assert!(!sequence.is_reorg_signal());
    }

    #[test]
    fn test_sequence_decode_unknown_event() {
        let mut data = Vec::new();
        // Hash bytes (32 bytes)
        let hash_bytes = [0x05; 32];
        data.extend_from_slice(&hash_bytes);
        // Unknown event type
        data.push(0x99);

        let mut cursor = Cursor::new(data);
        let sequence = Sequence::consensus_decode_from_finite_reader(&mut cursor).unwrap();

        assert_eq!(sequence.hash_bytes, hash_bytes);
        assert_eq!(sequence.event, SequenceEvent::Unknown(0x99));
        assert_eq!(sequence.mempool_seq, None);
        assert!(!sequence.is_block());
        assert!(!sequence.is_reorg_signal());
    }

    #[test]
    fn test_sequence_txid_conversion() {
        let mut hash_bytes = [0u8; 32];
        hash_bytes[0] = 0x01;
        hash_bytes[31] = 0xff;

        let sequence = Sequence {
            hash_bytes,
            event: SequenceEvent::TxAdded,
            mempool_seq: Some(123),
        };

        let txid = sequence.txid();
        // The hash should be reversed for txid
        let expected_hash = {
            let mut reversed = hash_bytes;
            reversed.reverse();
            reversed
        };
        assert_eq!(txid.as_raw_hash().as_byte_array(), &expected_hash);
    }

    #[test]
    fn test_sequence_block_hash_conversion() {
        let mut hash_bytes = [0u8; 32];
        hash_bytes[0] = 0x01;
        hash_bytes[31] = 0xff;

        let sequence = Sequence {
            hash_bytes,
            event: SequenceEvent::BlockConnected,
            mempool_seq: None,
        };

        let block_hash = sequence.block_hash();
        // The hash should be reversed for block hash
        let expected_hash = {
            let mut reversed = hash_bytes;
            reversed.reverse();
            reversed
        };
        assert_eq!(block_hash.as_raw_hash().as_byte_array(), &expected_hash);
    }

    #[test]
    fn test_sequence_display_block_events() {
        let hash_bytes = [0x01; 32];

        let block_connected = Sequence {
            hash_bytes,
            event: SequenceEvent::BlockConnected,
            mempool_seq: None,
        };

        let display_str = block_connected.to_string();
        assert!(display_str.contains("BlockConnected"));
        assert!(display_str.contains("hash="));
        assert!(!display_str.contains("mempool_seq"));

        let block_disconnected = Sequence {
            hash_bytes,
            event: SequenceEvent::BlockDisconnected,
            mempool_seq: None,
        };

        let display_str = block_disconnected.to_string();
        assert!(display_str.contains("BlockDisconnected"));
        assert!(display_str.contains("hash="));
        assert!(!display_str.contains("mempool_seq"));
    }

    #[test]
    fn test_sequence_display_tx_events() {
        let hash_bytes = [0x01; 32];

        let tx_added = Sequence {
            hash_bytes,
            event: SequenceEvent::TxAdded,
            mempool_seq: Some(12345),
        };

        let display_str = tx_added.to_string();
        assert!(display_str.contains("TxAdded"));
        assert!(display_str.contains("hash="));
        assert!(display_str.contains("mempool_seq=12345"));

        let tx_removed = Sequence {
            hash_bytes,
            event: SequenceEvent::TxRemoved,
            mempool_seq: Some(54321),
        };

        let display_str = tx_removed.to_string();
        assert!(display_str.contains("TxRemoved"));
        assert!(display_str.contains("hash="));
        assert!(display_str.contains("mempool_seq=54321"));
    }

    #[test]
    fn test_builder_creates_sequence_watcher_builder() {
        let shutdown = CancellationToken::new();
        let builder = builder("tcp://localhost:28332", shutdown.clone());

        // Test that the builder is the correct type
        assert_type::<SequenceWatcherBuilder>(builder);
    }

    #[test]
    fn test_builder_with_different_urls() {
        let shutdown = CancellationToken::new();

        let builder1 = builder("tcp://localhost:28332", shutdown.clone());
        let builder2 = builder("tcp://127.0.0.1:18332", shutdown.clone());
        let builder3 = builder("ipc:///tmp/bitcoin.sock", shutdown.clone());

        // All should be valid builders
        assert_type::<SequenceWatcherBuilder>(builder1);
        assert_type::<SequenceWatcherBuilder>(builder2);
        assert_type::<SequenceWatcherBuilder>(builder3);
    }

    #[test]
    fn test_type_aliases() {
        let shutdown = CancellationToken::new();
        let _builder: SequenceWatcherBuilder =
            SequenceWatcherBuilder::new("tcp://localhost:28332", shutdown);

        // Test Result type alias
        let ok_result: Result<i32, Error<Sequence>> = Ok(42);
        assert!(ok_result.is_ok());

        let err_result: Result<i32, Error<Sequence>> =
            Err(Error::ZmqError(zeromq::ZmqError::Other("test error")));
        assert!(err_result.is_err());
    }

    #[test]
    fn test_sequence_decode_insufficient_data() {
        // Test with insufficient data for hash
        let data = vec![0x01; 16]; // Only 16 bytes instead of 32
        let mut cursor = Cursor::new(data);
        let result = Sequence::consensus_decode_from_finite_reader(&mut cursor);
        assert!(result.is_err());

        // Test with hash but no event type
        let data = vec![0x01; 32]; // 32 bytes for hash but no event type
        let mut cursor = Cursor::new(data);
        let result = Sequence::consensus_decode_from_finite_reader(&mut cursor);
        assert!(result.is_err());

        // Test tx event without mempool sequence
        let mut data = Vec::new();
        data.extend_from_slice(&[0x01; 32]);
        data.push(b'A'); // TxAdded but no mempool sequence follows
        let mut cursor = Cursor::new(data);
        let result = Sequence::consensus_decode_from_finite_reader(&mut cursor);
        assert!(result.is_err());
    }
}
