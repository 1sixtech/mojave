use crate::rpc::RpcApiContext;

use ethrex_blockchain::Blockchain;
use ethrex_common::Bytes;
use ethrex_p2p::{
    peer_handler::PeerHandler,
    sync_manager::SyncManager,
    types::{Node, NodeRecord},
};
use ethrex_rpc::{GasTipEstimator, NodeData, RpcApiContext as L1Context, RpcErr};
use ethrex_storage::Store;
use ethrex_storage_rollup::StoreRollup;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, sync::Mutex as TokioMutex};
use tokio_util::sync::CancellationToken;
use tracing::info;

use mojave_rpc_core::types::Namespace;
use mojave_rpc_server::{RpcRegistry, RpcService};

pub const FILTER_DURATION: Duration = {
    if cfg!(test) {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(5 * 60)
    }
};

#[expect(clippy::too_many_arguments)]
pub async fn start_api(
    http_addr: SocketAddr,
    authrpc_addr: SocketAddr,
    storage: Store,
    blockchain: Arc<Blockchain>,
    jwt_secret: Bytes,
    local_p2p_node: Node,
    local_node_record: NodeRecord,
    syncer: SyncManager,
    peer_handler: PeerHandler,
    client_version: String,
    rollup_store: StoreRollup,
    shutdown_token: CancellationToken,
) -> Result<(), RpcErr> {
    let active_filters = Arc::new(Mutex::new(HashMap::new()));
    let context = RpcApiContext {
        l1_context: L1Context {
            storage,
            blockchain,
            active_filters: active_filters.clone(),
            syncer: Arc::new(syncer),
            peer_handler,
            node_data: NodeData {
                jwt_secret,
                local_p2p_node,
                local_node_record,
                client_version,
            },
            gas_tip_estimator: Arc::new(TokioMutex::new(GasTipEstimator::new())),
            log_filter_handler: None,
        },
        rollup_store,
    };

    // Periodically clean up the active filters for the filters endpoints.
    tokio::task::spawn(async move {
        let mut interval = tokio::time::interval(FILTER_DURATION);
        let filters = active_filters.clone();
        loop {
            interval.tick().await;
            tracing::info!("Running filter clean task");
            ethrex_rpc::clean_outdated_filters(filters.clone(), FILTER_DURATION);
            tracing::info!("Filter clean task complete");
        }
    });

    // Build RPC registry and service
    let mut registry: RpcRegistry<RpcApiContext> = RpcRegistry::new()
        .with_fallback(Namespace::Eth, |req, ctx: RpcApiContext| {
            Box::pin(ethrex_rpc::map_eth_requests(req, ctx.l1_context))
        });
    crate::rpc::handlers::register_moj_sendProofResponse(&mut registry);
    let service = RpcService::new(context.clone(), registry).with_permissive_cors();
    let http_router = service.router();
    let http_listener = TcpListener::bind(http_addr)
        .await
        .map_err(|error| RpcErr::Internal(error.to_string()))?;
    let http_server = axum::serve(http_listener, http_router)
        .with_graceful_shutdown(shutdown_token.cancelled_owned())
        .into_future();
    info!("Starting HTTP server at {http_addr}");

    info!("Not starting Auth-RPC server. The address passed as argument is {authrpc_addr}");

    let _ =
        tokio::try_join!(http_server).inspect_err(|e| info!("Error shutting down servers: {e:?}"));
    Ok(())
}
