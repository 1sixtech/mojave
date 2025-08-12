#[cfg(test)]
pub mod test_utils {
    use std::{net::SocketAddr, sync::Arc};
    use ethrex_rpc::EthClient;
    use mojave_client::MojaveClient;
    use tokio::sync::oneshot;
    use ethrex_storage::{Store, EngineType};
    use ethrex_blockchain::Blockchain;
    use ethrex_storage_rollup::StoreRollup;
    use ethrex_p2p::{types::Node, sync_manager::SyncManager, peer_handler::PeerHandler, types::NodeRecord};
    use mojave_full_node::rpc::start_api as start_api_full_node;
    use mojave_sequencer::rpc::start_api as start_api_sequencer;
    use mojave_chain_utils::unique_heap::AsyncUniqueHeap;
    
    pub const TEST_GENESIS: &str = include_str!("../../../test_data/genesis.json");
    pub const TEST_SEQUENCER_ADDR: &str = "127.0.0.1:8502";
    pub const TEST_NODE_ADDR: &str = "127.0.0.1:8500";
    
    // Placeholder functions - these need to be implemented based on your actual requirements
    fn example_p2p_node() -> Node {
        // This is a placeholder - implement according to your needs
        todo!("Implement example_p2p_node")
    }
    
    async fn example_rollup_store() -> StoreRollup {
        // This is a placeholder - implement according to your needs
        todo!("Implement example_rollup_store")
    }
    
    fn example_local_node_record() -> NodeRecord {
        // This is a placeholder - implement according to your needs
        todo!("Implement example_local_node_record")
    }
    

    pub async fn start_test_api_full_node(
        sequencer_addr: Option<SocketAddr>,
        http_addr: Option<SocketAddr>,
        authrpc_addr: Option<SocketAddr>,
    ) -> (MojaveClient, oneshot::Receiver<()>) {
        let http_addr = http_addr.unwrap_or(TEST_NODE_ADDR.parse().unwrap());
        let authrpc_addr = authrpc_addr.unwrap_or("127.0.0.1:8501".parse().unwrap());
        let storage =
            Store::new("", EngineType::InMemory).expect("Failed to create in-memory storage");
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
        let private_key = std::env::var("PRIVATE_KEY").unwrap();
        let client = MojaveClient::new(std::slice::from_ref(&url), &private_key).unwrap();
        let eth_client = EthClient::new(&url).unwrap();
        let block_queue = AsyncUniqueHeap::new();

        let rpc_api = start_api_full_node(
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
            eth_client,
            block_queue,
        );
        let (full_node_tx, full_node_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            full_node_tx.send(()).unwrap();
            rpc_api.await;
        });

        (client, full_node_rx)
    }

    pub async fn start_test_api_sequencer(
        node_urls: Option<Vec<SocketAddr>>,
        http_addr: Option<SocketAddr>,
        authrpc_addr: Option<SocketAddr>,
    ) -> (MojaveClient, oneshot::Receiver<()>) {
        let http_addr = http_addr.unwrap_or_else(|| TEST_SEQUENCER_ADDR.parse().unwrap());
        let authrpc_addr = authrpc_addr.unwrap_or_else(|| "127.0.0.1:8503".parse().unwrap());
        let storage =
            Store::new("", EngineType::InMemory).expect("Failed to create in-memory storage");
        storage
            .add_initial_state(serde_json::from_str(TEST_GENESIS).unwrap())
            .await
            .expect("Failed to build test genesis");
        let blockchain = Arc::new(Blockchain::default_with_store(storage.clone()));
        let jwt_secret = Default::default();
        let local_p2p_node = example_p2p_node();
        let rollup_store = example_rollup_store().await;
        let default_node_url = format!("http://{TEST_NODE_ADDR}");
        let node_urls: Vec<String> = match node_urls {
            Some(addrs) => addrs.iter().map(|addr| format!("http://{addr}")).collect(),
            None => vec![default_node_url.to_string()],
        };
        let private_key = std::env::var("PRIVATE_KEY").unwrap();
        let client = MojaveClient::new(&node_urls, &private_key).unwrap();

        let rpc_api = start_api_sequencer(
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
            rpc_api.await;
        });

        (client, sequencer_rx)
    }
}