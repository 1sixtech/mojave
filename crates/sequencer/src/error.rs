use crate::block_producer::BlockProducerError;
use ethrex_common::types::GenesisError;
use ethrex_rpc::RpcErr;
use ethrex_storage_rollup::RollupStoreError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to force remove the database: {0}")]
    ForceRemoveDatabase(std::io::Error),
    #[error(transparent)]
    Genesis(#[from] GenesisError),
    #[error(transparent)]
    StoreRollup(#[from] RollupStoreError),
    #[error(transparent)]
    BlockProducer(#[from] BlockProducerError),
    #[error(transparent)]
    Rpc(#[from] RpcErr),
    #[error(transparent)]
    MojaveClient(#[from] mojave_client::MojaveClientError),
}
