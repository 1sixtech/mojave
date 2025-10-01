use std::{
    fmt,
    path::{Path, PathBuf},
};

use ethrex_common::types::{Genesis, GenesisError};
use ethrex_p2p::types::Node;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

pub const TESTNET_GENESIS_PATH: &str = "data/testnet-genesis.json";
// Just a placeholder for now, will be replaced with real file later
const TESTNET_BOOTNODES_PATH: &str = "cmd/mojave/networks/testnet/bootnodes.json";

pub const MAINNET_GENESIS_PATH: &str = "cmd/mojave/networks/mainnet/genesis.json";
const MAINNET_BOOTNODES_PATH: &str = "cmd/mojave/networks/mainnet/bootnodes.json";

fn read_bootnodes(path: &str) -> Vec<Node> {
    // ethrex_p2p::rlpx::Message
    std::fs::File::open(path)
        .map_err(|e| {
            tracing::warn!(path, error = %e, "Failed to open bootnodes file; using empty list");
        })
        .and_then(|file| {
            serde_json::from_reader(file).map_err(|e| {
                tracing::warn!(path, error = %e, "Failed to parse bootnodes file; using empty list");
            })
        })
        .unwrap_or_default()
}

lazy_static! {
    pub static ref MAINNET_BOOTNODES: Vec<Node> = read_bootnodes(MAINNET_BOOTNODES_PATH);
    pub static ref TESTNET_BOOTNODES: Vec<Node> = read_bootnodes(TESTNET_BOOTNODES_PATH);
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum Network {
    #[default]
    DefaultNet,
    Mainnet,
    Testnet,
    GenesisPath(PathBuf),
}

impl From<&str> for Network {
    fn from(value: &str) -> Self {
        match value {
            "default" => Network::DefaultNet,
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
            Network::DefaultNet => {
                // should never happen, but just in case
                panic!("DefaultNet does not have a genesis path");
            }
            Network::Mainnet => Path::new(MAINNET_GENESIS_PATH),
            Network::Testnet => Path::new(TESTNET_GENESIS_PATH),
            Network::GenesisPath(s) => s,
        }
    }
    pub fn get_genesis(&self) -> Result<Genesis, GenesisError> {
        // If DefaultNet, construct a default genesis
        if let Network::DefaultNet = self {
            return Ok(Genesis::default());
        }
        Genesis::try_from(self.get_genesis_path())
    }

    pub fn get_bootnodes(&self) -> Vec<Node> {
        // TODO: add testnet and mainnet bootnodes
        vec![]
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Network::DefaultNet => write!(f, "default"),
            Network::Mainnet => write!(f, "mainnet"),
            Network::Testnet => write!(f, "testnet"),
            Network::GenesisPath(path) => write!(f, "{path:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn from_str_variants_map_correctly() {
        assert!(matches!(Network::from("default"), Network::DefaultNet));
        assert!(matches!(Network::from("mainnet"), Network::Mainnet));
        assert!(matches!(Network::from("testnet"), Network::Testnet));

        let network = Network::from("/tmp/genesis.json");
        match network {
            Network::GenesisPath(p) => assert_eq!(p, PathBuf::from("/tmp/genesis.json")),
            _ => panic!("expected GenesisPath"),
        }
    }

    #[test]
    fn from_pathbuf_becomes_genesispath() {
        let pathbuf = PathBuf::from("tmp/genesis.json");
        let network = Network::from(pathbuf.clone());
        match network {
            Network::GenesisPath(p) => assert_eq!(p, pathbuf),
            _ => panic!("expected GenesisPath"),
        }
    }

    #[test]
    fn display_formats_are_stable() {
        assert_eq!(format!("{}", Network::DefaultNet), "default");
        assert_eq!(format!("{}", Network::Mainnet), "mainnet");
        assert_eq!(format!("{}", Network::Testnet), "testnet");

        let network = Network::from("1six/mojave.json");
        let s = format!("{network}");
        assert!(s.contains("1six/mojave.json"));
    }

    #[test]
    #[should_panic(expected = "DefaultNet does not have a genesis path")]
    fn defaultnet_get_genesis_path_panics() {
        let _ = Network::DefaultNet.get_genesis_path();
    }

    #[test]
    fn invalid_path_get_genesis_err() {
        let network = Network::from("/does/not/exist.json");
        let err = network.get_genesis().unwrap_err();

        assert!(matches!(
            err,
            GenesisError::File(ref e) if e.kind() == std::io::ErrorKind::NotFound
        ));
    }
}
