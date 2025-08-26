use ethrex_blockchain::error::{ChainError, InvalidForkChoice};
use ethrex_l2_common::state_diff::StateDiffError;
use ethrex_storage::error::StoreError;
use ethrex_storage_rollup::RollupStoreError;
use ethrex_vm::EvmError;
use std::{num::TryFromIntError, time::SystemTimeError};
use tokio::sync::oneshot::error::RecvError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("BlockProducer failed because of a ChainError error: {0}")]
    ChainError(#[from] ChainError),
    #[error("BlockProducer could not finish the task")]
    Dropped(#[from] RecvError),
    #[error("BlockProducer failed because of a EvmError error: {0}")]
    EvmError(#[from] EvmError),
    #[error("Failed to encode AccountStateDiff: {0}")]
    FailedToEncodeAccountStateDiff(#[from] StateDiffError),
    #[error("BlockProducer failed because it failed to get data from: {0}")]
    FailedToGetDataFrom(String),
    #[error("BlockProducer failed to prepare PayloadAttributes timestamp: {0}")]
    FailedToGetSystemTime(#[from] SystemTimeError),
    #[error("Failed to build a block because the queue is full.")]
    Full,
    #[error("BlockProducer failed because of a InvalidForkChoice error: {0}")]
    InvalidForkChoice(#[from] InvalidForkChoice),
    #[error("BlockProducer failed because of a rollup store error: {0}")]
    RollupStoreError(#[from] RollupStoreError),
    #[error("BlockProducer stopped.")]
    Stopped,
    #[error("BlockProducer failed to retrieve a block from storage, data is None.")]
    StorageDataIsNone,
    #[error("BlockProducer failed because of a store error: {0}")]
    StoreError(#[from] StoreError),
    #[error("BlockProducer failed because interval does not fit in u64")]
    TryInto(#[from] TryFromIntError),
}
