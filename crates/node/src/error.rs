use ethrex_common::types::GenesisError;
use ethrex_rpc::clients::EthClientError;
use ethrex_storage_rollup::RollupStoreError;

pub type Result<T> = std::result::Result<T, Error>;
pub use ethrex_rpc::RpcErr as RpcError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // This shouldn't exist https://github.com/lambdaclass/ethrex/issues/4167
    #[error("{0}")]
    Custom(String),
    #[error(transparent)]
    EthClient(#[from] EthClientError),
    #[error("Failed to force remove the database: {0}")]
    ForceRemoveDatabase(std::io::Error),
    #[error(transparent)]
    Genesis(#[from] GenesisError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Failed to initiate the node: {0}")]
    NodeInit(std::io::Error),
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error(transparent)]
    Secp256k1(#[from] secp256k1::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Store(#[from] ethrex_storage::error::StoreError),
    #[error(transparent)]
    StoreRollup(#[from] RollupStoreError),
}
