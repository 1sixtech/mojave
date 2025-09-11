use clap::ValueEnum;
use ethrex_blockchain::Blockchain;
use ethrex_common::types::Genesis;
pub use ethrex_p2p::types::Node;
use ethrex_p2p::{
    kademlia::Kademlia, peer_handler::PeerHandler, sync_manager::SyncManager, types::NodeRecord,
};
use ethrex_storage::Store;
use ethrex_storage_rollup::StoreRollup;
use mojave_utils::network::Network;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Serialize, Deserialize)]
pub struct NodeConfigFile {
    pub known_peers: Vec<Node>,
    pub node_record: NodeRecord,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum SyncMode {
    #[default]
    Full,
    Snap,
}

impl From<SyncMode> for ethrex_p2p::sync::SyncMode {
    fn from(mode: SyncMode) -> Self {
        match mode {
            SyncMode::Full => ethrex_p2p::sync::SyncMode::Full,
            SyncMode::Snap => ethrex_p2p::sync::SyncMode::Snap,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeOptions {
    pub network: Network,
    pub bootnodes: Vec<Node>,
    pub syncmode: SyncMode,
    pub sponsorable_addresses_file_path: Option<String>,
    pub datadir: String,
    pub force: bool,
    pub metrics_addr: String,
    pub metrics_port: String,
    pub metrics_enabled: bool,
    pub http_addr: String,
    pub http_port: String,
    pub authrpc_addr: String,
    pub authrpc_port: String,
    pub authrpc_jwtsecret: String,
    pub p2p_enabled: bool,
    pub p2p_addr: String,
    pub p2p_port: String,
    pub discovery_addr: String,
    pub discovery_port: String,
}

// TODO: set proper defaults for work without config
impl Default for NodeOptions {
    fn default() -> Self {
        Self {
            http_addr: "0.0.0.0".to_owned(),
            http_port: "8545".to_owned(),
            authrpc_addr: "localhost".to_owned(),
            authrpc_port: "8551".to_owned(),
            authrpc_jwtsecret: "jwt.hex".to_owned(),
            p2p_enabled: true,
            p2p_addr: "0.0.0.0".to_owned(),
            p2p_port: "30303".to_owned(),
            discovery_addr: "0.0.0.0".to_owned(),
            discovery_port: "30303".to_owned(),
            network: Network::DefaultNet,
            bootnodes: vec![],
            datadir: "./mojave/mojave-node".to_owned(),
            syncmode: SyncMode::Full,
            sponsorable_addresses_file_path: None,
            metrics_addr: "0.0.0.0".to_owned(),
            metrics_port: "9090".to_owned(),
            metrics_enabled: false,
            force: false,
        }
    }
}

pub struct MojaveNode {
    pub data_dir: String,
    pub genesis: Genesis,
    pub store: Store,
    pub rollup_store: StoreRollup,
    pub blockchain: Arc<Blockchain>,
    pub cancel_token: CancellationToken,
    pub local_p2p_node: Node,
    pub local_node_record: Arc<Mutex<NodeRecord>>,
    pub syncer: SyncManager,
    pub peer_table: Kademlia,
    pub peer_handler: PeerHandler,
}
