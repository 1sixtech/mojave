use ethrex::{
    initializers::{get_local_node_record, get_signer, init_blockchain, init_store},
    utils::{NodeConfigFile, get_client_version, read_jwtsecret_file, store_node_config_file},
};
use ethrex_blockchain::BlockchainType;
use ethrex_p2p::{network::peer_table, peer_handler::PeerHandler, sync_manager::SyncManager};
use ethrex_rpc::EthClient;
use ethrex_storage_rollup::{EngineTypeRollup, StoreRollup};
use ethrex_vm::EvmEngine;
use mojave_chain_utils::{
    initializer::{
        get_authrpc_socket_addr, get_http_socket_addr, get_local_p2p_node, resolve_data_dir,
    },
    logging::init_logging,
    unique_heap::AsyncUniqueHeap,
};
use mojave_full_node::{
    cli::{Cli, Command},
    error::Error,
    rpc::start_api,
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = Cli::run();
    init_logging(cli.log_level);
    match cli.command {
        Command::Init {
            options,
            full_node_options,
        } => {
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

            let signer = get_signer(&data_dir);

            let local_p2p_node = get_local_p2p_node(&options, &signer);
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
                options.syncmode.clone(),
                cancel_token.clone(),
                blockchain.clone(),
                store.clone(),
            )
            .await;

            let rpc_shutdown = CancellationToken::new();
            let eth_client = EthClient::new(&full_node_options.sequencer_address)?;
            start_api(
                get_http_socket_addr(&options),
                get_authrpc_socket_addr(&options),
                store,
                blockchain,
                read_jwtsecret_file(&options.authrpc_jwtsecret),
                local_p2p_node,
                local_node_record.lock().await.clone(),
                syncer,
                peer_handler,
                get_client_version(),
                rollup_store.clone(),
                eth_client,
                AsyncUniqueHeap::new(),
                rpc_shutdown.clone(),
            )
            .await?;

            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Shutting down the full node..");
                    rpc_shutdown.cancel();
                    let node_config_path = PathBuf::from(data_dir).join("node_config.json");
                    tracing::info!("Storing config at {:?}...", node_config_path);
                    cancel_token.cancel();
                    let node_config = NodeConfigFile::new(peer_table, local_node_record.lock().await.clone()).await;
                    store_node_config_file(node_config, node_config_path).await;
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    tracing::info!("Successfully shut down the full node.");
                }
            }
        }
    }
    Ok(())
}
