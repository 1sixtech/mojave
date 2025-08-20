use ethrex_common::types::GenesisError;
use ethrex_rpc::{RpcErr, clients::EthClientError};
use ethrex_storage_rollup::RollupStoreError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    EthClient(#[from] EthClientError),
    #[error("Failed to force remove the database: {0}")]
    ForceRemoveDatabase(std::io::Error),
    #[error(transparent)]
    Genesis(#[from] GenesisError),
    #[error(transparent)]
    Rpc(#[from] RpcErr),
    #[error(transparent)]
    StoreRollup(#[from] RollupStoreError),
}
