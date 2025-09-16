use ethrex_common::types::GenesisError;
use ethrex_p2p::network::NetworkError;
use ethrex_rpc::clients::EthClientError;
use ethrex_storage_rollup::RollupStoreError;
use local_ip_address::Error as LocalIPError;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // This shouldn't exist https://github.com/lambdaclass/ethrex/issues/4167
    #[error("{0}")]
    Custom(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error(transparent)]
    EthClient(#[from] EthClientError),
    #[error("Failed to force remove the database: {0}")]
    ForceRemoveDatabase(std::io::Error),
    #[error(transparent)]
    Genesis(#[from] GenesisError),
    #[error(transparent)]
    Hex(#[from] hex::FromHexError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Failed to initiate the node: {0}")]
    NodeInit(std::io::Error),
    #[error(transparent)]
    Rpc(#[from] mojave_utils::rpc::error::Error),
    #[error("EthrexNextwork error: {0}")]
    EthrexNextwork(#[from] NetworkError),
    #[error(transparent)]
    LocalIP(#[from] LocalIPError),
    #[error(transparent)]
    Secp256k1(#[from] secp256k1::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Store(#[from] ethrex_storage::error::StoreError),
    #[error(transparent)]
    StoreRollup(#[from] RollupStoreError),
}
