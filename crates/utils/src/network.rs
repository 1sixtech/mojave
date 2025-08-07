use std::{
    fmt,
    path::{Path, PathBuf},
};

use ethrex_common::types::{Genesis, GenesisError};
use ethrex_p2p::types::Node;
use lazy_static::lazy_static;

pub const TESTNET_GENESIS_PATH: &str = "cmd/mojave/networks/testnet/genesis.json";
const TESTNET_BOOTNODES_PATH: &str = "cmd/mojave/networks/testnet/bootnodes.json";

pub const MAINNET_GENESIS_PATH: &str = "cmd/mojave/networks/mainnet/genesis.json";
const MAINNET_BOOTNODES_PATH: &str = "cmd/mojave/networks/mainnet/bootnodes.json";

lazy_static! {
    pub static ref MAINNET_BOOTNODES: Vec<Node> = serde_json::from_reader(
        std::fs::File::open(MAINNET_BOOTNODES_PATH).expect("Failed to open mainnet bootnodes file")
    )
    .expect("Failed to parse mainnet bootnodes file");
    pub static ref TESTNET_BOOTNODES: Vec<Node> = serde_json::from_reader(
        std::fs::File::open(TESTNET_BOOTNODES_PATH).expect("Failed to open testnet bootnodes file")
    )
    .expect("Failed to parse testnet bootnodes file");
}

#[derive(Debug, Clone, Default)]
pub enum Network {
    #[default]
    Mainnet,
    Testnet,
    GenesisPath(PathBuf),
}

impl From<&str> for Network {
    fn from(value: &str) -> Self {
        match value {
            "mainnet" => Network::Mainnet,
            "testnet" => Network::Testnet,
            s => Network::GenesisPath(PathBuf::from(s)),
        }
    }
}

impl From<PathBuf> for Network {
    fn from(value: PathBuf) -> Self {
        Network::GenesisPath(value)
    }
}

impl Network {
    pub fn get_genesis_path(&self) -> &Path {
        match self {
            Network::Mainnet => Path::new(MAINNET_GENESIS_PATH),
            Network::Testnet => Path::new(TESTNET_GENESIS_PATH),
            Network::GenesisPath(s) => s,
        }
    }
    pub fn get_genesis(&self) -> Result<Genesis, GenesisError> {
        Genesis::try_from(self.get_genesis_path())
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Network::Mainnet => write!(f, "Mainnet"),
            Network::Testnet => write!(f, "Testnet"),
            Network::GenesisPath(path) => write!(f, "{path:?}"),
        }
    }
}
