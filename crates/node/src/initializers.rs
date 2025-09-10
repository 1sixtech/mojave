use crate::{
    error::{Error, Result},
    utils::read_node_config_file,
};

use ethrex_blockchain::{Blockchain, BlockchainType};
use ethrex_common::types::Genesis;
use ethrex_p2p::types::{Node, NodeRecord};
use ethrex_storage::{EngineType, Store};
use rand::rngs::OsRng;
use secp256k1::SecretKey;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::info;

/// Opens a new or pre-existing Store and loads the initial state provided by the network
pub async fn init_store(data_dir: &str, genesis: Genesis) -> Result<Store> {
    let store = open_store(data_dir)?;
    store.add_initial_state(genesis).await?;
    Ok(store)
}

/// Initializes a pre-existing Store
pub async fn load_store(data_dir: &str) -> Result<Store> {
    let store = open_store(data_dir)?;
    store.load_initial_state().await?;
    Ok(store)
}

/// Opens a pre-existing Store or creates a new one
pub fn open_store(data_dir: &str) -> Result<Store> {
    let path = PathBuf::from(data_dir);
    if path.ends_with("memory") {
        Ok(Store::new(data_dir, EngineType::InMemory)?)
    } else {
        let engine_type = EngineType::Libmdbx;
        Ok(Store::new(data_dir, engine_type)?)
    }
}

pub fn init_blockchain(store: Store, blockchain_type: BlockchainType) -> Arc<Blockchain> {
    info!("Initiating blockchain");
    Blockchain::new(store, blockchain_type, false).into()
}

pub fn get_signer(data_dir: &str) -> Result<SecretKey> {
    // Get the signer from the default directory, create one if the key file is not present.
    let key_path = Path::new(data_dir).join("node.key");
    match fs::read(key_path.clone()) {
        Ok(content) => Ok(SecretKey::from_slice(&content)?),
        Err(_) => {
            info!(
                "Key file not found, creating a new key and saving to {:?}",
                key_path
            );
            if let Some(parent) = key_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let signer = SecretKey::new(&mut OsRng);
            fs::write(&key_path, signer.secret_bytes())?;
            key_path.metadata()?.permissions().set_mode(0o600);
            Ok(signer)
        }
    }
}

pub fn get_local_node_record(
    data_dir: &str,
    local_p2p_node: &Node,
    signer: &SecretKey,
) -> Result<NodeRecord> {
    let config_file = PathBuf::from(data_dir.to_owned() + "/node_config.json");

    match read_node_config_file(config_file) {
        Ok(ref mut config) => {
            Ok(
                NodeRecord::from_node(local_p2p_node, config.node_record.seq + 1, signer)
                    .map_err(Error::Custom)?,
            )
        }
        Err(_) => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Ok(NodeRecord::from_node(local_p2p_node, timestamp, signer).map_err(Error::Custom)?)
        }
    }
}
