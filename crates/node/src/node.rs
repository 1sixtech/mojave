use crate::{
    error::Error,
    initializers::{get_local_node_record, get_signer, init_blockchain, init_store},
    rpc::start_api,
    types::{MojaveNode, NodeOptions},
    utils::{
        NodeConfigFile, get_authrpc_socket_addr, get_http_socket_addr, get_local_p2p_node,
        read_jwtsecret_file, resolve_data_dir, store_node_config_file,
    },
};
use ethrex_blockchain::BlockchainType;
use ethrex_p2p::{network::peer_table, peer_handler::PeerHandler, sync_manager::SyncManager};
use ethrex_rpc::EthClient;
use ethrex_storage_rollup::{EngineTypeRollup, StoreRollup};
use ethrex_vm::EvmEngine;
use mojave_utils::unique_heap::AsyncUniqueHeap;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

impl MojaveNode {
    pub async fn init(options: &NodeOptions) -> Result<Self, Box<dyn std::error::Error>> {
        let data_dir = resolve_data_dir(&options.datadir);
        tracing::info!("Data directory resolved to: {:?}", data_dir);

        if options.force {
            tracing::info!("Force removing the database at {:?}", data_dir);
            std::fs::remove_dir_all(&data_dir).map_err(Error::ForceRemoveDatabase)?;
        }

        let genesis = options.network.get_genesis()?;

        let store = init_store(&data_dir, genesis.clone()).await;
        tracing::info!("Successfully initialized the database.");

        let rollup_store = StoreRollup::new(&data_dir, EngineTypeRollup::InMemory)?;
        rollup_store.init().await?;
        tracing::info!("Successfully initialized the rollup database.");

        let blockchain = init_blockchain(EvmEngine::LEVM, store.clone(), BlockchainType::L2);

        let cancel_token = tokio_util::sync::CancellationToken::new();

        let signer = get_signer(&data_dir)?;

        let local_p2p_node = get_local_p2p_node(
            &options.discovery_addr,
            &options.discovery_port,
            &options.p2p_addr,
            &options.p2p_port,
            &signer,
        );
        let local_node_record = Arc::new(Mutex::new(get_local_node_record(
            &data_dir,
            &local_p2p_node,
            &signer,
        )));

        let peer_table = peer_table(local_p2p_node.node_id());
        let peer_handler = PeerHandler::new(peer_table.clone());

        // Create SyncManager
        let syncer = SyncManager::new(
            peer_handler.clone(),
            options.syncmode.into(),
            cancel_token.clone(),
            blockchain.clone(),
            store.clone(),
        )
        .await;

        Ok(MojaveNode {
            data_dir,
            genesis,
            store,
            rollup_store,
            blockchain,
            cancel_token,
            local_p2p_node,
            local_node_record,
            syncer,
            peer_table,
            peer_handler,
        })
    }

    pub async fn run(self, options: &NodeOptions) -> Result<(), Box<dyn std::error::Error>> {
        let rpc_shutdown = CancellationToken::new();
        let eth_client = EthClient::new("http://127.0.0.1")?;
        let jwt_secret = read_jwtsecret_file(&options.authrpc_jwtsecret)?;
        start_api(
            get_http_socket_addr(&options.http_addr, &options.http_port),
            get_authrpc_socket_addr(&options.authrpc_addr, &options.authrpc_port),
            self.store,
            self.blockchain,
            jwt_secret,
            self.local_p2p_node,
            self.local_node_record.lock().await.clone(),
            self.syncer,
            self.peer_handler,
            get_client_version(),
            self.rollup_store.clone(),
            eth_client,
            AsyncUniqueHeap::new(),
            rpc_shutdown.clone(),
        )
        .await?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutting down the full node..");
                rpc_shutdown.cancel();
                let node_config_path = PathBuf::from(self.data_dir).join("node_config.json");
                tracing::info!("Storing config at {:?}...", node_config_path);
                self.cancel_token.cancel();
                let node_config = NodeConfigFile::new(self.peer_table, self.local_node_record.lock().await.clone()).await;
                store_node_config_file(node_config, node_config_path).await;
                tokio::time::sleep(Duration::from_secs(1)).await;
                tracing::info!("Successfully shut down the full node.");
            }
        }

        Ok(())
    }
}

pub fn get_client_version() -> String {
    format!("{}/v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"),)
}
