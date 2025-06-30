use crate::sync::SyncClient;
use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use ethrex_blockchain::Blockchain;
use ethrex_common::{Bytes, types::Transaction};
use ethrex_p2p::{
    peer_handler::PeerHandler,
    sync_manager::SyncManager,
    types::{Node, NodeRecord},
};
use ethrex_rpc::{
    GasTipEstimator, NodeData, RpcApiContext as L1Context, RpcErr, RpcNamespace, RpcRequestWrapper,
    types::transaction::SendRawTransactionRequest,
    utils::{RpcRequest, RpcRequestId},
};
use ethrex_storage::Store;
use ethrex_storage_rollup::StoreRollup;
use serde_json::Value;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, sync::Mutex as TokioMutex};
use tower_http::cors::CorsLayer;
use tracing::{debug, info};

pub const FILTER_DURATION: Duration = Duration::from_secs(300);

#[derive(Clone, Debug)]
pub struct RpcApiContext {
    pub l1_context: L1Context,
    pub rollup_store: StoreRollup,
    pub sync_client: SyncClient,
}

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
    sync_client: SyncClient,
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
        },
        rollup_store,
        sync_client,
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

    // All request headers allowed.
    // All methods allowed.
    // All origins allowed.
    // All headers exposed.
    let cors = CorsLayer::permissive();

    let http_router = Router::new()
        .route("/", post(handle_http_request))
        .layer(cors)
        .with_state(context.clone());
    let http_listener = TcpListener::bind(http_addr)
        .await
        .map_err(|error| RpcErr::Internal(error.to_string()))?;
    let http_server = axum::serve(http_listener, http_router)
        .with_graceful_shutdown(ethrex_rpc::shutdown_signal())
        .into_future();
    info!("Starting HTTP server at {http_addr}");

    info!("Not starting Auth-RPC server. The address passed as argument is {authrpc_addr}");

    let _ =
        tokio::try_join!(http_server).inspect_err(|e| info!("Error shutting down servers: {e:?}"));

    Ok(())
}

async fn handle_http_request(
    State(service_context): State<RpcApiContext>,
    body: String,
) -> Result<Json<Value>, StatusCode> {
    let res = match serde_json::from_str::<RpcRequestWrapper>(&body) {
        Ok(RpcRequestWrapper::Single(request)) => {
            let res = map_http_requests(&request, service_context).await;
            ethrex_rpc::rpc_response(request.id, res).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        Ok(RpcRequestWrapper::Multiple(requests)) => {
            let mut responses = Vec::new();
            for req in requests {
                let res = map_http_requests(&req, service_context.clone()).await;
                responses.push(
                    ethrex_rpc::rpc_response(req.id, res).map_err(|_| StatusCode::BAD_REQUEST)?,
                );
            }
            serde_json::to_value(responses).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        Err(_) => ethrex_rpc::rpc_response(
            RpcRequestId::String("".to_string()),
            Err(ethrex_rpc::RpcErr::BadParams(
                "Invalid request body".to_string(),
            )),
        )
        .map_err(|_| StatusCode::BAD_REQUEST)?,
    };
    Ok(Json(res))
}

/// TODO: Export map_ns_requests in different branch.
async fn map_http_requests(req: &RpcRequest, context: RpcApiContext) -> Result<Value, RpcErr> {
    match req.namespace() {
        Ok(RpcNamespace::Eth) => map_eth_requests(req, context).await,
        Ok(RpcNamespace::Admin) => map_admin_requests(req, context.l1_context),
        Ok(RpcNamespace::Debug) => map_debug_requests(req, context.l1_context).await,
        Ok(RpcNamespace::Web3) => map_web3_requests(req, context.l1_context),
        Ok(RpcNamespace::Net) => map_net_requests(req, context.l1_context),
        Ok(RpcNamespace::Mempool) => map_mempool_requests(req, context.l1_context).await,
        Ok(RpcNamespace::Engine) => Err(RpcErr::Internal(
            "Engine namespace not allowed in map_http_requests".to_owned(),
        )),
        Err(rpc_err) => Err(rpc_err),
    }
}
