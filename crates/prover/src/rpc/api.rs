use crate::{
    job::{JobRecord, JobStore},
    notifier::Notifier,
    rpc::{ProverRpcContext, tasks::spawn_proof_worker},
};
use mojave_client::types::ProofResponse;
use mojave_rpc_server::{RpcRegistry, RpcService};
use mojave_utils::rpc::error::{Error, Result};

use std::sync::Arc;
use tokio::{net::TcpListener, sync::mpsc};
use tracing::info;

pub async fn start_api(
    aligned_mode: bool,
    http_addr: &str,
    _private_key: &str,
    queue_capacity: usize,
) -> Result<()> {
    let (job_sender, job_receiver) = mpsc::channel::<JobRecord>(queue_capacity);
    let (proof_sender, mut proof_receiver) = mpsc::channel::<ProofResponse>(queue_capacity);
    let notifier = Notifier::new(proof_sender);
    let context = Arc::new(ProverRpcContext {
        aligned_mode,
        job_store: JobStore::default(),
        job_sender,
        notifier,
    });
    tracing::info!(aligned_mode = %aligned_mode, "Prover RPC context initialized");

    let mut registry: RpcRegistry<Arc<ProverRpcContext>> = RpcRegistry::new();
    crate::rpc::handlers::register_moj_sendProofInput(&mut registry);
    crate::rpc::handlers::register_moj_getPendingJobIds(&mut registry);
    crate::rpc::handlers::register_moj_getProof(&mut registry);
    let service = RpcService::new(context.clone(), registry).with_permissive_cors();
    let http_router = service.router();
    let http_listener = TcpListener::bind(http_addr)
        .await
        .map_err(|error| Error::Internal(error.to_string()))?;
    tracing::info!(addr = %http_addr, "HTTP server bound");
    let http_server = axum::serve(http_listener, http_router).into_future();
    info!("Starting HTTP server at {http_addr}");

    // Start the proof worker in the background.
    let proof_worker_handle = spawn_proof_worker(context, job_receiver);
    tracing::info!("Proof worker task spawned");

    let _ = tokio::try_join!(
        async {
            http_server
                .await
                .map_err(|e| Error::Internal(e.to_string()))
        },
        async {
            proof_worker_handle
                .await
                .map_err(|e| Error::Internal(e.to_string()))
        },
        //spawn dummy consumer
        async {
            tokio::spawn({
                async move {
                    loop {
                        match proof_receiver.recv().await {
                            Some(proof) => {
                                tracing::info!("Receive proof: {proof:?}")
                            }
                            None => {
                                tracing::warn!("Receiver dropped");
                                break;
                            }
                        }
                    }
                }
            })
            .await
            .map_err(|e| Error::Internal(e.to_string()))
        }
    )
    .inspect_err(|e| tracing::error!("Error shutting down server:{e:?}"));

    Ok(())
}
