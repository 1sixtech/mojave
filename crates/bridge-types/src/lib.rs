//! Shared type definitions for Mojave Bitcoin Bridge
//!
//! This crate provides common types used across the bridge infrastructure,
//! including UTXO management, Bitcoin-related types, and event definitions.

pub mod bitcoin;
pub mod events;
pub mod utxo;

pub use bitcoin::*;
pub use events::*;
pub use utxo::*;

/// Result type for bridge operations
pub type Result<T> = std::result::Result<T, BridgeError>;

/// Bridge-specific errors
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Invalid UTXO: {0}")]
    InvalidUtxo(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u64, available: u64 },

    #[error("Bitcoin error: {0}")]
    Bitcoin(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Hex decoding error: {0}")]
    HexDecode(#[from] hex::FromHexError),
}
