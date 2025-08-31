use crate::rpc::{
    context::RpcApiContext,
    requests::{SendBroadcastBlockRequest, SendRawTransactionRequest},
    tasks::{spawn_block_ingestion_task, spawn_block_processing_task, spawn_filter_cleanup_task},
    types::{OrderedBlock, PendingHeap},
};
use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use ethrex_blockchain::Blockchain;
use ethrex_common::Bytes;
use ethrex_p2p::{
    peer_handler::PeerHandler,
    sync_manager::SyncManager,
    types::{Node, NodeRecord},
};
use ethrex_rpc::{
    EthClient, GasTipEstimator, NodeData, RpcApiContext as L1Context, RpcErr, RpcRequestWrapper,
    utils::{RpcRequest, RpcRequestId},
};
use ethrex_storage::Store;
use ethrex_storage_rollup::StoreRollup;
use mojave_utils::{
    rpc::{
        error::{Error, Result},
        resolve_namespace, rpc_response,
        types::{MojaveRequestMethods, Namespace},
    },
    unique_heap::AsyncUniqueHeap,
};
use serde_json::{Value, from_str, to_string};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::{net::TcpListener, sync::Mutex as TokioMutex};
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use tracing::info;

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
    eth_client: EthClient,
    block_queue: AsyncUniqueHeap<OrderedBlock, u64>,
    shutdown_token: CancellationToken,
) -> Result<()> {
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
        eth_client,
        block_queue,
        pending_signed_blocks: PendingHeap::new(),
    };

    // Periodically clean up the active filters for the filters endpoints.
    let filter_handle = spawn_filter_cleanup_task(active_filters.clone(), shutdown_token.clone());
    let block_handle = spawn_block_processing_task(context.clone(), shutdown_token.clone());
    let block_ingestion_handle =
        spawn_block_ingestion_task(context.clone(), shutdown_token.clone());

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

    let _ = tokio::try_join!(
        async {
            http_server
                .await
                .map_err(|e| RpcErr::Internal(e.to_string()))
        },
        async {
            filter_handle
                .await
                .map_err(|e| RpcErr::Internal(e.to_string()))
        },
        async {
            block_handle
                .await
                .map_err(|e| RpcErr::Internal(e.to_string()))
        },
        async {
            block_ingestion_handle
                .await
                .map_err(|e| RpcErr::Internal(e.to_string()))
        }
    )
    .inspect_err(|e| info!("Error shutting down servers: {e:?}"));

    Ok(())
}

async fn handle_http_request(
    State(service_context): State<RpcApiContext>,
    body: String,
) -> core::result::Result<Json<Value>, StatusCode> {
    let res = match serde_json::from_str::<RpcRequestWrapper>(&body) {
        Ok(RpcRequestWrapper::Single(request)) => {
            let res = map_http_requests(&request, service_context).await;
            rpc_response(request.id, res).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        Ok(RpcRequestWrapper::Multiple(requests)) => {
            let mut responses = Vec::new();
            for req in requests {
                let res = map_http_requests(&req, service_context.clone()).await;
                responses.push(rpc_response(req.id, res).map_err(|_| StatusCode::BAD_REQUEST)?);
            }
            serde_json::to_value(responses).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        Err(_) => rpc_response(
            RpcRequestId::String("".to_string()),
            Err(RpcErr::BadParams("Invalid request body".to_string())),
        )
        .map_err(|_| StatusCode::BAD_REQUEST)?,
    };
    Ok(Json(res))
}

async fn map_http_requests(req: &RpcRequest, context: RpcApiContext) -> Result<Value> {
    match resolve_namespace(req) {
        Ok(Namespace::Eth) => map_eth_requests(req, context).await,
        Ok(Namespace::Mojave) => map_mojave_requests(req, context).await,
        Ok(_) => Err(Error::MethodNotFound(req.method.clone())),
        Err(error) => Err(error),
    }
}

async fn map_eth_requests(req: &RpcRequest, context: RpcApiContext) -> Result<Value> {
    match req.method.as_str() {
        "eth_sendRawTransaction" => SendRawTransactionRequest::call(req, context).await,
        _others => ethrex_rpc::map_eth_requests(req, context.l1_context).await,
    }
}

async fn map_mojave_requests(req: &RpcRequest, context: RpcApiContext) -> Result<Value> {
    let method = from_str(&req.method)?;
    match method {
        MojaveRequestMethods::SendBroadcastBlock => {
            SendBroadcastBlockRequest::call(req, context).await
        }
        others => Err(Error::MethodNotFound(to_string(&others)?)),
    }
}
