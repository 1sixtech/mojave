use bitcoin::taproot::TaprootBuilderError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BatchSubmitterError {
    #[error("Bitcoin RPC error: {0}")]
    BitcoinRPCError(#[from] bitcoincore_rpc::Error),
    #[error("Error building taproot")]
    TaprootError(#[from] TaprootBuilderError),
    #[error("Anyhow error: {0}")]
    AnyhowError(#[from] anyhow::Error),
    #[error("Wallet error: {0}")]
    WalletError(String),
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Batch notifier error: {0}")]
    BatchNotifierError(#[from] tokio::sync::mpsc::error::TrySendError<u64>),
}
