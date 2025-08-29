use std::sync::Arc;

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use serde_json::Value;
use tokio::{net::TcpListener, sync::mpsc};
use tower_http::cors::CorsLayer;
use tracing::info;

use ethrex_rpc::{
    RpcErr, RpcRequestWrapper,
    utils::{RpcRequest, RpcRequestId},
};

use mojave_client::MojaveClient;
use mojave_utils::rpc::rpc_response;

use crate::rpc::{
    ProverRpcContext,
    requests::{GetJobIdRequest, GetProofRequest, SendProofInputRequest},
    tasks::spawn_proof_worker,
    types::{JobRecord, JobStore},
};

pub async fn start_api(
    aligned_mode: bool,
    http_addr: &str,
    private_key: &str,
    queue_capacity: usize,
) -> Result<(), RpcErr> {
    let (job_sender, job_receiver) = mpsc::channel::<JobRecord>(queue_capacity);
    let context = Arc::new(ProverRpcContext {
        aligned_mode,
        job_store: JobStore::default(),
        sender: job_sender,
    });
    tracing::info!(aligned_mode = %aligned_mode, "Prover RPC context initialized");

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
    tracing::info!(addr = %http_addr, "HTTP server bound");
    let http_server = axum::serve(http_listener, http_router).into_future();
    info!("Starting HTTP server at {http_addr}");

    let client = MojaveClient::builder()
        .private_key(private_key.to_string())
        .build()
        .map_err(|err| RpcErr::Internal(err.to_string()))?;
    tracing::info!("MojaveClient initialized");

    // Start the proof worker in the background.
    let proof_worker_handle = spawn_proof_worker(context, job_receiver, client);
    tracing::info!("Proof worker task spawned");

    let _ = tokio::try_join!(
        async {
            http_server
                .await
                .map_err(|e| RpcErr::Internal(e.to_string()))
        },
        async {
            proof_worker_handle
                .await
                .map_err(|e| RpcErr::Internal(e.to_string()))
        }
    )
    .inspect_err(|e| tracing::error!("Error shutting down server:{e:?}"));

    Ok(())
}

async fn handle_http_request(
    State(service_context): State<Arc<ProverRpcContext>>,
    body: String,
) -> Result<Json<Value>, StatusCode> {
    tracing::trace!(len = body.len(), "Received HTTP request body");
    let res = match serde_json::from_str::<RpcRequestWrapper>(&body) {
        Ok(RpcRequestWrapper::Single(request)) => {
            tracing::debug!(method = %request.method, "Handling single RPC request");
            let res = map_http_requests(&request, service_context).await;
            rpc_response(request.id, res).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        Ok(RpcRequestWrapper::Multiple(requests)) => {
            tracing::debug!(req_count = requests.len(), "Handling batch RPC requests");
            let mut responses = Vec::new();
            for req in requests {
                let res = map_http_requests(&req, service_context.clone()).await;
                responses.push(rpc_response(req.id, res).map_err(|_| StatusCode::BAD_REQUEST)?);
            }
            serde_json::to_value(responses).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        Err(_) => {
            tracing::error!("Invalid request body");
            rpc_response(
                RpcRequestId::String("".to_string()),
                Err(RpcErr::BadParams("Invalid request body".to_string())),
            )
            .map_err(|_| StatusCode::BAD_REQUEST)?
        }
    };
    Ok(Json(res))
}

async fn map_http_requests(
    req: &RpcRequest,
    context: Arc<ProverRpcContext>,
) -> Result<Value, RpcErr> {
    tracing::debug!(method = %req.method, "Dispatching RPC request");
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
    tracing::debug!(method = %req.method, "Handling Mojave namespace request");
    match req.method.as_str() {
        "mojave_sendProofInput" => SendProofInputRequest::call(req, context).await,
        "mojave_getJobId" => GetJobIdRequest::call(req, context).await,
        "mojave_getProof" => GetProofRequest::call(req, context).await,
        _others => Err(RpcErr::MethodNotFound(req.method.clone())),
    }
}

pub enum RpcNamespace {
    Mojave,
}

impl RpcNamespace {
    pub fn resolve_namespace(request: &RpcRequest) -> Result<Self, RpcErr> {
        let mut parts = request.method.split('_');
        let Some(namespace) = parts.next() else {
            return Err(RpcErr::MethodNotFound(request.method.clone()));
        };
        match namespace {
            "mojave" => Ok(Self::Mojave),
            _others => Err(RpcErr::MethodNotFound(request.method.to_owned())),
        }
    }
}
