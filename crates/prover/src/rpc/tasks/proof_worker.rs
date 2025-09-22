use std::sync::Arc;

use ethrex_prover_lib::{backend::Backend, prove, to_batch_proof};
use ethrex_rpc::RpcErr;
use mojave_client::types::{ProofResponse, ProofResult};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::rpc::{ProverRpcContext, types::JobRecord};

pub(crate) fn spawn_proof_worker(
    ctx: Arc<ProverRpcContext>,
    mut receiver: mpsc::Receiver<JobRecord>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("Proof worker started");
        loop {
            match receiver.recv().await {
                Some(job) => {
                    tracing::debug!(job_id = %job.job_id.as_ref(), "Worker received job");

                    let batch_number = job.prover_data.batch_number;
                    let program_input = job.prover_data.input;
                    let try_generate_proof = prove(Backend::Exec, program_input, ctx.aligned_mode)
                        .and_then(|output| to_batch_proof(output, ctx.aligned_mode))
                        .map_err(|err| {
                            RpcErr::Internal(format!("Error while generate proof: {err:}"))
                        });

                    let result = match try_generate_proof {
                        Ok(proof) => {
                            tracing::info!(job_id = %job.job_id.as_ref(), %batch_number, "Proof generated");
                            ProofResult::Proof(proof)
                        }
                        Err(e) => {
                            tracing::error!(job_id = %job.job_id.as_ref(), %batch_number, error = %e, "Proof generation failed");
                            ProofResult::Error(e.to_string())
                        }
                    };

                    let proof_response = ProofResponse {
                        job_id: job.job_id,
                        batch_number,
                        result,
                    };

                    ctx.job_store
                        .upsert_proof(&proof_response.job_id, proof_response.clone())
                        .await;

                    todo!("Send proof to sequencer")
                }
                None => {
                    tracing::info!("Proof worker channel closed; stopping");
                    break;
                }
            }
        }
    })
}
