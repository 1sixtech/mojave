use ethrex_blockchain::error::ChainError;
use ethrex_l2::sequencer::errors::ExecutionCacheError;
use ethrex_storage::error::StoreError;
use ethrex_storage_rollup::RollupStoreError;
use tokio::task::JoinError;

pub type Result<T> = core::result::Result<T, Error>;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("ProofCoordinator connection failed: {0}")]
    Connection(#[from] std::io::Error),
    #[error("ProofCoordinator failed to send transaction: {0}")]
    FailedToVerifyProofOnChain(String),
    #[error("ProofCoordinator failed to access Store: {0}")]
    FailedAccessingStore(#[from] StoreError),
    #[error("ProverServer failed to access RollupStore: {0}")]
    FailedAccessingRollupStore(#[from] RollupStoreError),
    #[error("ProofCoordinator failed to retrieve block from storage, data is None.")]
    StorageDataIsNone,
    #[error("ProofCoordinator failed to create ExecutionWitness: {0}")]
    FailedToCreateExecutionWitness(#[from] ChainError),
    #[error("ProofCoordinator JoinError: {0}")]
    JoinError(#[from] JoinError),
    #[error("Failed to build the client: {0}")]
    Client(#[from] mojave_client::error::Error),
    #[error("ProofCoordinator failed: {0}")]
    Custom(String),
    #[error("ProofCoordinator failed to get data from Store: {0}")]
    ItemNotFoundInStore(String),
    #[error("Unexpected Error: {0}")]
    Internal(String),
    #[error("ProofCoordinator encountered a ExecutionCacheError")]
    ExecutionCacheError(#[from] ExecutionCacheError),
    #[error("ProofCoordinator encountered a BlobsBundleError: {0}")]
    BlobsBundleError(#[from] ethrex_common::types::BlobsBundleError),
    #[error("Failed to execute command: {0}")]
    Command(std::io::Error),
    #[error("Missing blob for batch {0}")]
    MissingBlob(u64),
    #[error("Proof failed for batch {0}: {1}")]
    ProofFailed(u64, String),
}
