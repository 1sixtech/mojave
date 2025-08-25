use ethrex_blockchain::Blockchain;
use ethrex_common::H512;
use ethrex_p2p::{
    peer_handler::PeerHandler,
    sync_manager::SyncManager,
    types::{Node, NodeRecord},
};
use ethrex_rpc::EthClient;
use ethrex_storage::{EngineType, Store};
use ethrex_storage_rollup::{EngineTypeRollup, StoreRollup};
use k256::ecdsa::SigningKey;
use mojave_block_producer::rpc::start_api as start_api_block_producer;
use mojave_client::MojaveClient;
use mojave_node_lib::rpc::start_api as start_api_node;
use mojave_utils::unique_heap::AsyncUniqueHeap;
use std::{net::SocketAddr, str::FromStr, sync::Arc};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub const TEST_GENESIS: &str = include_str!("../../../tests/mock-genesis.json");
pub const TEST_SEQUENCER_ADDR: &str = "127.0.0.1:8502";
pub const TEST_NODE_ADDR: &str = "127.0.0.1:8500";

pub fn example_p2p_node() -> Node {
    let public_key_1 = H512::from_str("d860a01f9722d78051619d1e2351aba3f43f943f6f00718d1b9baa4101932a1f5011f16bb2b1bb35db20d6fe28fa0bf09636d26a87d31de9ec6203eeedb1f666").unwrap();
    Node::new("127.0.0.1".parse().unwrap(), 30303, 30303, public_key_1)
}

pub async fn example_rollup_store() -> StoreRollup {
    let rollup_store =
        StoreRollup::new(".", EngineTypeRollup::InMemory).expect("Failed to create StoreRollup");
    rollup_store
        .init()
        .await
        .expect("Failed to init rollup store");
    rollup_store
}

pub fn example_local_node_record() -> NodeRecord {
    let public_key_1 = H512::from_str("d860a01f9722d78051619d1e2351aba3f43f943f6f00718d1b9baa4101932a1f5011f16bb2b1bb35db20d6fe28fa0bf09636d26a87d31de9ec6203eeedb1f666").unwrap();
    let node = Node::new("127.0.0.1".parse().unwrap(), 30303, 30303, public_key_1);
    let k256_signer = SigningKey::random(&mut rand::rngs::OsRng);
    let secret_key = secp256k1::SecretKey::from_slice(&k256_signer.to_bytes()).unwrap();

    NodeRecord::from_node(&node, 1, &secret_key).unwrap()
}

pub async fn start_test_api_node(
    sequencer_addr: Option<SocketAddr>,
    http_addr: Option<SocketAddr>,
    authrpc_addr: Option<SocketAddr>,
) -> (EthClient, oneshot::Receiver<()>) {
    let http_addr = http_addr.unwrap_or(TEST_NODE_ADDR.parse().unwrap());
    let authrpc_addr = authrpc_addr.unwrap_or("127.0.0.1:8501".parse().unwrap());
    let storage = Store::new("", EngineType::InMemory).expect("Failed to create in-memory storage");
    storage
        .add_initial_state(serde_json::from_str(TEST_GENESIS).unwrap())
        .await
        .expect("Failed to build test genesis");
    let blockchain = Arc::new(Blockchain::default_with_store(storage.clone()));
    let jwt_secret = Default::default();
    let local_p2p_node = example_p2p_node();
    let rollup_store = example_rollup_store().await;
    let sequencer_addr = match sequencer_addr {
        Some(addr) => addr,
        None => TEST_SEQUENCER_ADDR.parse().unwrap(),
    };
    let url = format!("http://{sequencer_addr}");
    let eth_client = EthClient::new(&url).unwrap();
    let block_queue = AsyncUniqueHeap::new();
    let shutdown_token = CancellationToken::new();
    let rpc_api = start_api_node(
        http_addr,
        authrpc_addr,
        storage,
        blockchain,
        jwt_secret,
        local_p2p_node,
        example_local_node_record(),
        SyncManager::dummy(),
        PeerHandler::dummy(),
        "ethrex/test".to_string(),
        rollup_store,
        eth_client.clone(),
        block_queue,
        shutdown_token,
    );
    let (full_node_tx, full_node_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        full_node_tx.send(()).unwrap();
        rpc_api.await.unwrap()
    });

    (eth_client, full_node_rx)
}

pub async fn start_test_api_sequencer(
    http_addr: Option<SocketAddr>,
    authrpc_addr: Option<SocketAddr>,
) -> (MojaveClient, oneshot::Receiver<()>) {
    let http_addr = http_addr.unwrap_or_else(|| TEST_SEQUENCER_ADDR.parse().unwrap());
    let authrpc_addr = authrpc_addr.unwrap_or_else(|| "127.0.0.1:8503".parse().unwrap());
    let storage = Store::new("", EngineType::InMemory).expect("Failed to create in-memory storage");
    storage
        .add_initial_state(serde_json::from_str(TEST_GENESIS).unwrap())
        .await
        .expect("Failed to build test genesis");
    let blockchain = Arc::new(Blockchain::default_with_store(storage.clone()));
    let jwt_secret = Default::default();
    let local_p2p_node = example_p2p_node();
    let rollup_store = example_rollup_store().await;
    let private_key = std::env::var("PRIVATE_KEY").unwrap();
    let client = MojaveClient::new(&private_key).unwrap();
    let rpc_api = start_api_block_producer(
        http_addr,
        authrpc_addr,
        storage,
        blockchain,
        jwt_secret,
        local_p2p_node,
        example_local_node_record(),
        SyncManager::dummy(),
        PeerHandler::dummy(),
        "ethrex/test".to_string(),
        rollup_store,
    );

    let (sequencer_tx, sequencer_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        sequencer_tx.send(()).unwrap();
        rpc_api.await.unwrap()
    });

    (client, sequencer_rx)
}
