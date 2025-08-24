use crate::utils::read_node_config_file;

use ethrex_blockchain::{Blockchain, BlockchainType};
use ethrex_common::types::Genesis;
use ethrex_p2p::types::{Node, NodeRecord};
use ethrex_storage::{EngineType, Store};
use ethrex_vm::EvmEngine;
use rand::rngs::OsRng;
use secp256k1::SecretKey;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::info;

/// Opens a new or pre-existing Store and loads the initial state provided by the network
pub async fn init_store(data_dir: &str, genesis: Genesis) -> Store {
    let store = open_store(data_dir);
    store
        .add_initial_state(genesis)
        .await
        .expect("Failed to create genesis block");
    store
}

/// Initializes a pre-existing Store
pub async fn load_store(data_dir: &str) -> Store {
    let store = open_store(data_dir);
    store
        .load_initial_state()
        .await
        .expect("Failed to load store");
    store
}

/// Opens a pre-existing Store or creates a new one
pub fn open_store(data_dir: &str) -> Store {
    let path = PathBuf::from(data_dir);
    if path.ends_with("memory") {
        Store::new(data_dir, EngineType::InMemory).expect("Failed to create Store")
    } else {
        let engine_type = EngineType::Libmdbx;
        Store::new(data_dir, engine_type).expect("Failed to create Store")
    }
}

pub fn init_blockchain(
    evm_engine: EvmEngine,
    store: Store,
    blockchain_type: BlockchainType,
) -> Arc<Blockchain> {
    info!("Initiating blockchain with EVM: {}", evm_engine);
    Blockchain::new(evm_engine, store, blockchain_type).into()
}

pub fn get_signer(data_dir: &str) -> SecretKey {
    // Get the signer from the default directory, create one if the key file is not present.
    let key_path = Path::new(data_dir).join("node.key");
    match fs::read(key_path.clone()) {
        Ok(content) => SecretKey::from_slice(&content).expect("Signing key could not be created."),
        Err(_) => {
            info!(
                "Key file not found, creating a new key and saving to {:?}",
                key_path
            );
            if let Some(parent) = key_path.parent() {
                fs::create_dir_all(parent).expect("Key file path could not be created.")
            }
            let signer = SecretKey::new(&mut OsRng);
            fs::write(key_path, signer.secret_bytes())
                .expect("Newly created signer could not be saved to disk.");
            signer
        }
    }
}

pub fn get_local_node_record(
    data_dir: &str,
    local_p2p_node: &Node,
    signer: &SecretKey,
) -> NodeRecord {
    let config_file = PathBuf::from(data_dir.to_owned() + "/node_config.json");

    match read_node_config_file(config_file) {
        Ok(ref mut config) => {
            NodeRecord::from_node(local_p2p_node, config.node_record.seq + 1, signer)
                .expect("Node record could not be created from local node")
        }
        Err(_) => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            NodeRecord::from_node(local_p2p_node, timestamp, signer)
                .expect("Node record could not be created from local node")
        }
    }
}
