use crate::{error::Result, utils::read_node_config_file_async};

use ethrex_blockchain::{Blockchain, BlockchainOptions, BlockchainType};
use ethrex_common::types::Genesis;
use ethrex_p2p::types::{Node, NodeRecord};
use ethrex_storage::{EngineType, Store};
use rand::rngs::OsRng;
use secp256k1::SecretKey;
use std::{
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
    let options = BlockchainOptions {
        r#type: blockchain_type,
        ..Default::default()
    };
    Blockchain::new(store, options).into()
}

pub async fn get_signer(data_dir: &str) -> Result<SecretKey> {
    // Get the signer from the default directory, create one if the key file is not present.
    let key_path = Path::new(data_dir).join("node.key");
    match tokio::fs::read(key_path.clone()).await {
        Ok(content) => Ok(SecretKey::from_slice(&content)?),
        Err(_) => {
            info!(
                "Key file not found, creating a new key and saving to {:?}",
                key_path
            );
            if let Some(parent) = key_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let signer = SecretKey::new(&mut OsRng);
            tokio::fs::write(&key_path, signer.secret_bytes()).await?;
            let mut perms = tokio::fs::metadata(&key_path).await?.permissions();
            perms.set_mode(0o600);
            tokio::fs::set_permissions(&key_path, perms).await?;
            Ok(signer)
        }
    }
}

pub async fn get_local_node_record(
    data_dir: &str,
    local_p2p_node: &Node,
    signer: &SecretKey,
) -> Result<NodeRecord> {
    let config_file = PathBuf::from(data_dir.to_owned() + "/node_config.json");

    match read_node_config_file_async(config_file).await {
        Ok(ref mut config) => Ok(NodeRecord::from_node(
            local_p2p_node,
            config.node_record.seq + 1,
            signer,
        )?),
        Err(_) => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Ok(NodeRecord::from_node(local_p2p_node, timestamp, signer)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env::temp_dir, fs, str::FromStr};

    #[tokio::test]
    async fn get_signer_creates_key_with_0600_and_is_idempotent() {
        let test_dir = temp_dir().join("get_signer_test");
        let key_file_path = test_dir.join("node.key");

        let key_path_str = key_file_path.to_str().unwrap();
        let secret_key1 = get_signer(test_dir.to_str().unwrap()).await.unwrap();
        let key_bytes = fs::read(key_path_str).unwrap();
        assert_eq!(key_bytes.len(), 32);

        // Check permission 0600 on Unix(Owner can read and write, others have no permission)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Trim to last 3 octal digits:
            // mask out file type and special bits (setuid/setgid/sticky), keeping only the 9 permission bits (rwxrwxrwx).
            let mode = fs::metadata(key_path_str).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        let mtime_before = fs::metadata(key_path_str).unwrap().modified().unwrap();

        let secret_key2 = get_signer(test_dir.to_str().unwrap())
            .await
            .expect("reuse key");
        assert_eq!(secret_key1.secret_bytes(), secret_key2.secret_bytes());

        let mtime_after = fs::metadata(key_path_str).unwrap().modified().unwrap();

        assert_eq!(mtime_before, mtime_after);

        // cleanup
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[tokio::test]
    async fn get_signer_creates_missing_parent_dirs() {
        // Ensure nested directories are created
        let test_dir = temp_dir().join("get_signer_nested");
        let nested = test_dir.join("a/b/c");
        let nested_str = nested.to_str().unwrap();

        let key_path = nested.join("node.key");
        assert!(!key_path.exists());

        get_signer(nested_str)
            .await
            .expect("create with nested dirs");
        assert!(key_path.exists());

        // cleanup
        let _ = fs::remove_dir_all(test_dir);
    }

    #[tokio::test]
    async fn get_signer_fails_on_corrupted_existing_key() {
        let test_dir = temp_dir().join("get_signer_corrupted");
        let key_path = test_dir.join("node.key");
        fs::create_dir_all(test_dir.clone()).unwrap();

        // Write an invalid key (wrong size)
        fs::write(&key_path, [0u8; 16]).unwrap();

        let err = get_signer(test_dir.to_str().unwrap())
            .await
            .expect_err("must not accept invalid key");
        // Short and tolerant check on error content
        let msg = format!("{err}");
        assert!(msg.to_lowercase().contains("secret key"));

        // Ensure file was not replaced
        let meta = fs::metadata(&key_path).unwrap();
        assert_eq!(meta.len(), 16, "must not rewrite corrupted key");

        // cleanup
        let _ = fs::remove_dir_all(test_dir);
    }

    #[tokio::test]
    async fn get_local_node_record_uses_timestamp_when_no_config() {
        use secp256k1::{PublicKey, Secp256k1};

        let dir = temp_dir().join("tmp_node_record");
        let signer = get_signer(dir.to_str().unwrap()).await.unwrap();

        let secp = Secp256k1::new();
        let pub_key = PublicKey::from_secret_key(&secp, &signer);
        let uncompressed = pub_key.serialize_uncompressed();
        let pubkey_hex = hex::encode(&uncompressed[1..]); // drop 0x04

        let enode = format!("enode://{pubkey_hex}@127.0.0.1:30303");
        let local = Node::from_str(&enode).expect("valid local enode");

        let rec = get_local_node_record(dir.to_str().unwrap(), &local, &signer)
            .await
            .expect("node record");
        assert!(rec.seq > 0);
        drop(rec);

        // cleanup
        let _ = fs::remove_dir(&dir);
    }
}
