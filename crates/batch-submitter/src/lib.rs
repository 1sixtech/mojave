use thiserror::Error;

#[derive(Debug, Error)]
pub enum BatchSubmitterError {
    #[error("Bitcoin RPC error: {0}")]
    BitcoinRPCError(#[from] bitcoincore_rpc::Error),
    #[error("Anyhow error: {0}")]
    AnyhowError(#[from] anyhow::Error),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Inscribing error: {0}")]
    InscribingError(String),
    #[error("Wallet error: {0}")]
    WalletError(String),
    #[error("Transaction error: {0}")]
    TransactionError(String),
    #[error("Other error: {0}")]
    Other(String),
}

pub mod config;
pub mod writer;
