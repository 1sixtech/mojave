use ethrex_blockchain::error::ChainError;
use ethrex_l2::sequencer::errors::ExecutionCacheError;
use ethrex_l2_common::prover::BatchProof;
use ethrex_storage::error::StoreError;
use ethrex_storage_rollup::RollupStoreError;
use mojave_prover::ProverData;
use tokio::{sync::mpsc::error::SendError, task::JoinError};

#[allow(clippy::large_enum_variant)]
#[derive(Debug, thiserror::Error)]
pub enum ProofCoordinatorError {
    #[error("ProofCoordinator connection failed: {0}")]
    ConnectionError(#[from] std::io::Error),
    #[error("ProofCoordinator failed to send transaction: {0}")]
    FailedToVerifyProofOnChain(String),
    #[error("ProofCoordinator failed to access Store: {0}")]
    FailedAccessingStore(#[from] StoreError),
    #[error("ProverServer failed to access RollupStore: {0}")]
    FailedAccessingRollupStore(#[from] RollupStoreError),
    #[error("ProofCoordinator failed to retrieve block from storaga, data is None.")]
    StorageDataIsNone,
    #[error("ProofCoordinator failed to create ExecutionWitness: {0}")]
    FailedToCreateExecutionWitness(#[from] ChainError),
    #[error("ProofCoordinator JoinError: {0}")]
    JoinError(#[from] JoinError),
    #[error("ProofCoordinator failed: {0}")]
    Custom(String),
    #[error("ProofCoordinator failed to write to TcpStream: {0}")]
    WriteError(String),
    #[error("ProofCoordinator failed to get data from Store: {0}")]
    ItemNotFoundInStore(String),
    #[error("Unexpected Error: {0}")]
    InternalError(String),
    #[error("ProofCoordinator encountered a ExecutionCacheError")]
    ExecutionCacheError(#[from] ExecutionCacheError),
    #[error("ProofCoordinator encountered a BlobsBundleError: {0}")]
    BlobsBundleError(#[from] ethrex_common::types::BlobsBundleError),
    #[error("Failed to execute command: {0}")]
    ComandError(std::io::Error),
    #[error("Missing blob for batch {0}")]
    MissingBlob(u64),
    #[error("ProofCoordinator prover data send error {0}")]
    ProverDataSendError(SendError<ProverData>),
    #[error("ProofCoordinator proof sender error {0}")]
    ProofSendError(SendError<(BatchProof, u64)>),
}
