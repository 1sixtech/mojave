use ethrex_blockchain::error::ChainError;
use ethrex_common::types::{BlobsBundleError, batch::Batch};
use ethrex_l2_common::{
    privileged_transactions::PrivilegedTransactionError, state_diff::StateDiffError,
};
use ethrex_storage::error::StoreError;
use ethrex_storage_rollup::RollupStoreError;
use ethrex_vm::EvmError;
use std::{num::TryFromIntError, time::SystemTimeError};

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("BatchProducer failed because of a ChainError error: {0}")]
    ChainError(#[from] ChainError),
    #[error("BatchProducer failed because of a EvmError error: {0}")]
    EvmError(#[from] EvmError),
    #[error("Failed to encode AccountStateDiff: {0}")]
    FailedToEncodeAccountStateDiff(#[from] StateDiffError),
    #[error("BatchProducer failed because of a rollup store error: {0}")]
    RollupStoreError(#[from] RollupStoreError),
    #[error("BatchProducer failed because of a store error: {0}")]
    StoreError(#[from] StoreError),
    #[error("BatchProducer failed to prepare timestamp: {0}")]
    FailedToGetSystemTime(#[from] SystemTimeError),
    #[error("BatchProducer failed because interval does not fit in u64")]
    TryInto(#[from] TryFromIntError),
    #[error("Retrieval Error: {0}")]
    RetrievalError(String),
    #[error("Failed to get information from storage: {0}")]
    FailedToGetInformationFromStorage(String),
    #[error("Failed to generate blobs bundle: {0}")]
    FailedToGenerateBlobsBundle(#[from] BlobsBundleError),
    #[error("Unreachable code reached: {0}")]
    Unreachable(String),
    #[error("Privileged Transaction error: {0}")]
    PrivilegedTransactionError(#[from] PrivilegedTransactionError),
    #[error("Send error on channel: {0}")]
    BroadcastError(#[from] Box<tokio::sync::broadcast::error::SendError<Batch>>),
}

impl From<tokio::sync::broadcast::error::SendError<Batch>> for Error {
    fn from(err: tokio::sync::broadcast::error::SendError<Batch>) -> Self {
        Error::BroadcastError(Box::new(err))
    }
}
