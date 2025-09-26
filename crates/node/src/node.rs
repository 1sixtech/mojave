use crate::{
    error::{Error, Result},
    initializers::{get_local_node_record, get_signer, init_blockchain, init_store},
    p2p::network::start_network,
    rpc::{context::RpcApiContext, start_api},
    types::{MojaveNode, NodeConfigFile, NodeOptions},
    utils::{
        get_authrpc_socket_addr, get_http_socket_addr, get_local_p2p_node, read_jwtsecret_file,
        resolve_data_dir, store_node_config_file,
    },
};
use ethrex_blockchain::BlockchainType;
use ethrex_p2p::{
    network::peer_table, peer_handler::PeerHandler, rlpx::l2::l2_connection::P2PBasedContext,
    sync_manager::SyncManager,
};
use ethrex_storage_rollup::{EngineTypeRollup, StoreRollup};
use mojave_rpc_server::RpcRegistry;
use mojave_utils::unique_heap::AsyncUniqueHeap;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tokio_util::task::TaskTracker;

impl MojaveNode {
    pub async fn init(options: &NodeOptions) -> Result<Self> {
        let data_dir = resolve_data_dir(&options.datadir).await?;
        tracing::info!("Data directory resolved to: {:?}", data_dir);

        if options.force {
            tracing::info!("Force removing the database at {:?}", data_dir);
            tokio::fs::remove_dir_all(&data_dir)
                .await
                .map_err(Error::ForceRemoveDatabase)?;
        }

        let genesis = options.network.get_genesis()?;

        let store = init_store(&data_dir, genesis.clone()).await?;
        tracing::info!("Successfully initialized the database.");

        let rollup_store = StoreRollup::new(&data_dir, EngineTypeRollup::InMemory)?;
        rollup_store.init().await?;
        tracing::info!("Successfully initialized the rollup database.");

        let blockchain = init_blockchain(store.clone(), BlockchainType::L2);

        let cancel_token = tokio_util::sync::CancellationToken::new();

        let signer = get_signer(&data_dir).await?;

        let local_p2p_node = get_local_p2p_node(
            &options.discovery_addr,
            &options.discovery_port,
            &options.p2p_addr,
            &options.p2p_port,
            &signer,
        )
        .await?;
        let local_node_record = Arc::new(Mutex::new(
            get_local_node_record(&data_dir, &local_p2p_node, &signer).await?,
        ));

        let peer_table = peer_table();
        let peer_handler = PeerHandler::new(peer_table.clone());

        let based_context = Some(P2PBasedContext {
            store_rollup: rollup_store.clone(),
            committer_key: Arc::new(signer),
        });
        blockchain.set_synced();

        let tracker = TaskTracker::new();

        start_network(
            options.bootnodes.clone(),
            &options.network,
            &data_dir,
            local_p2p_node.clone(),
            local_node_record.clone(),
            signer,
            peer_table.clone(),
            store.clone(),
            tracker,
            blockchain.clone(),
            based_context,
        )
        .await?;

        // Create SyncManager
        let syncer = Arc::new(
            SyncManager::new(
                peer_handler.clone(),
                options.syncmode.into(),
                cancel_token.clone(),
                blockchain.clone(),
                store.clone(),
                data_dir.clone(),
            )
            .await,
        );

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

    pub async fn run(self, options: &NodeOptions, registry: RpcRegistry<RpcApiContext>) -> Result<()> {
        let rpc_shutdown = self.cancel_token.child_token();
        let jwt_secret = read_jwtsecret_file(&options.authrpc_jwtsecret).await?;
        let api_task = start_api(
            get_http_socket_addr(&options.http_addr, &options.http_port).await?,
            get_authrpc_socket_addr(&options.authrpc_addr, &options.authrpc_port).await?,
            self.store,
            self.blockchain,
            jwt_secret,
            self.local_p2p_node,
            self.local_node_record.lock().await.clone(),
            self.syncer,
            self.peer_handler,
            get_client_version(),
            self.rollup_store.clone(),
            AsyncUniqueHeap::new(),
            rpc_shutdown.clone(),
            registry,
        );
        tokio::pin!(api_task);
        tokio::select! {
            res = &mut api_task => {
                if let Err(error) = res {
                    tracing::error!("API task returned error: {}", error);
                }
            }
            _ = mojave_utils::signal::wait_for_shutdown_signal() => {
                tracing::info!("Shutting down the full node..");
                let node_config_path = PathBuf::from(self.data_dir).join("node_config.json");
                tracing::info!("Storing config at {:?}...", node_config_path);
                self.cancel_token.cancel();
                let node_config = NodeConfigFile::new(self.peer_table, self.local_node_record.lock().await.clone()).await;
                store_node_config_file(node_config, node_config_path).await;

                if let Err(_elapsed) = tokio::time::timeout(std::time::Duration::from_secs(10), api_task).await {
                    tracing::warn!("Timed out waiting for API to stop");
                }
                tracing::info!("Successfully shut down the full node.");
            }
        }

        Ok(())
    }
}

pub fn get_client_version() -> String {
    format!("{}/v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"),)
}
