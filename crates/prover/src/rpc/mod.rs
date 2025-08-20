mod proof;
use proof::*;
mod types;
use types::*;

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use ethrex_rpc::{utils::{RpcRequest, RpcRequestId}, RpcErr, RpcRequestWrapper};
use serde_json::Value;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{Mutex, RwLock},
};
use tower_http::cors::CorsLayer;
use tracing::info;

use mojave_chain_utils::rpc::rpc_response;

pub const FILTER_DURATION: Duration = {
    if cfg!(test) {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(5 * 60)
    }
};

pub async fn start_api(
    aligned_mode: bool,
    http_addr: SocketAddr,
    // client_version: String,
) -> Result<(), RpcErr> {
    let context = Arc::new(ProverRpcContext {
        aligned_mode,
        job_queue: Mutex::new(HashMap::new()),
        proofs: Mutex::new(HashMap::new()),
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
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        })
        .into_future();
    info!("Starting HTTP server at {http_addr}");

    if let Err(e) = http_server.await.map_err(|e| RpcErr::Internal(e.to_string())) {
        info!("Error shutting down server: {e:?}");
        return Err(e);
    }
    
    Ok(())
}

async fn handle_http_request(
    State(service_context): State<Arc<ProverRpcContext>>,
    body: String,
) -> Result<Json<Value>, StatusCode> {
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

async fn map_http_requests(req: &RpcRequest, context: Arc<ProverRpcContext>) -> Result<Value, RpcErr> {
    match RpcNamespace::resolve_namespace(req) {
        Ok(RpcNamespace::Mojave) => map_mojave_requests(req, context).await,
        Err(err) => Err(err),
    }
}

/// Leave this unimplemented for now.
pub async fn map_mojave_requests(
    req: &RpcRequest,
    context: Arc<ProverRpcContext>,
) -> Result<Value, RpcErr> {
    match req.method.as_str() {
        "mojave_sendProofInput" => SendProofInputRequest::call(req, context).await,
        "mojave_getJobID" => GetJobIdRequest::call(req, context).await,
        "mojave_getProof" => GetProofRequest::call(req, context).await,
        _others => Err(RpcErr::MethodNotFound(req.method)),
    }
}

