use reqwest::Url;
use serde_json::{Value, json};
use tracing::info;
use std::sync::Arc;
use tiny_keccak::{Hasher, Keccak};
use tokio::sync::mpsc;

use ethrex_prover_lib::{backends::Backend, prove, to_batch_proof};
use ethrex_rpc::{RpcErr, utils::RpcRequest};
use zkvm_interface::io::ProgramInput;

use mojave_chain_utils::prover_types::{ProofResponse, ProverData};
use mojave_client::MojaveClient;

use crate::rpc::{ProverRpcContext, types::JobRecord};

pub struct SendProofInputRequest {
    prover_data: ProverData,
    sequencer_addr: Url,
}

impl SendProofInputRequest {
    fn get_proof_input(req: &Option<Vec<Value>>) -> Result<Self, RpcErr> {
        let params = req
            .as_ref()
            .ok_or(RpcErr::BadParams("No param provided".to_owned()))?;

        if params.len() != 2 {
            return Err(RpcErr::BadParams(format!(
                "Expected 2 params, got {}",
                params.len()
            )));
        };

        let prover_data =
            serde_json::from_value::<ProverData>(params[0].clone()).map_err(|err| {
                RpcErr::BadParams(format!("Can't parse 1st param as ProverData: {}", err))
            })?;
        let sequencer_addr =
            serde_json::from_value::<Url>(params[1].clone()).map_err(|err| {
                RpcErr::BadParams(format!("Can't parse 2nd param as Url: {}", err))
            })?;

        Ok(SendProofInputRequest {
            prover_data,
            sequencer_addr,
        })
    }

    pub async fn call(req: &RpcRequest, ctx: Arc<ProverRpcContext>) -> Result<Value, RpcErr> {
        let proof_input = Self::get_proof_input(&req.params)?;

        let job_id = Self::calculate_job_id(&proof_input.prover_data.input)?;
        if ctx.get_job_status(&job_id).await.is_some() {
            return Err(RpcErr::BadParams("This batch already requested".to_owned()));
        }

        let record = JobRecord {
            job_id: job_id.clone(),
            prover_data: Arc::new(proof_input.prover_data),
            sequencer_url: proof_input.sequencer_addr.clone(),
        };

        ctx.insert_job_sender(record).await?;

        Ok(json!({"job_id": job_id}))
    }

    fn calculate_job_id(prover_input: &ProgramInput) -> Result<String, RpcErr> {
        let serialized_program_input = bincode::serialize(prover_input)
            .map_err(|err| RpcErr::Internal(format!("Error to serialize program input{:}", err)))?;

        let mut hasher = Keccak::v256();
        hasher.update(&serialized_program_input.as_slice());
        let mut hash = [0_u8; 32];
        hasher.finalize(&mut hash);

        Ok(hex::encode(&hash))
    }
}

pub struct GetJobIdRequest;
impl GetJobIdRequest {
    pub async fn call(req: &RpcRequest, ctx: Arc<ProverRpcContext>) -> Result<Value, RpcErr> {
        if let Some(param) = req.params.as_ref() {
            return Err(RpcErr::BadParams(format!(
                "Expected 0 params, got {}",
                param.len()
            )));
        };

        Ok(json!(ctx.get_pending_jobs().await))
    }
}

pub struct GetProofRequest;
impl GetProofRequest {
    fn get_job_id(req: &Option<Vec<Value>>) -> Result<String, RpcErr> {
        let param = req
            .as_ref()
            .ok_or(RpcErr::BadParams("No param provided".to_owned()))?;

        if param.len() != 1 {
            return Err(RpcErr::BadParams(format!(
                "Expected 1 params, got {}",
                param.len()
            )));
        };

        let job_id_value = param
            .first()
            .ok_or(RpcErr::BadParams("Job Id didn't provided".to_owned()))?;

        let job_id = serde_json::from_value::<String>(job_id_value.clone())?;

        Ok(job_id)
    }

    pub async fn call(req: &RpcRequest, ctx: Arc<ProverRpcContext>) -> Result<Value, RpcErr> {
        let job_id = Self::get_job_id(&req.params)?;

        let proof = ctx
            .get_proof_by_id(&job_id)
            .await
            .ok_or(RpcErr::Internal(format!(
                "No proof exist with job id {}",
                &job_id
            )))?;

        Ok(json!(proof))
    }
}

pub async fn start_proof_worker(
    ctx: Arc<ProverRpcContext>,
    mut receiver: mpsc::Receiver<JobRecord>,
) {
    // TODO: implement sign while sending proof?
    let client = MojaveClient::new("0x1").expect("Error to start client to send proof back!");
    loop {
        match receiver.recv().await {
            Some(job) => {
                let job_id = job.job_id.clone();
                let (batch_number, program_input) = match Arc::try_unwrap(job.prover_data) {
                    Ok(prover_data) => (prover_data.batch_number, prover_data.input),
                    Err(_) => {
                        let proof_response = ProofResponse {
                            job_id: job_id.clone(),
                            batch_number: 0,
                            error: Some(
                                "Internal error: Error while unwrap prover data".to_owned(),
                            ),
                            batch_proof: None,
                        };

                        ctx.upsert_proof(&job_id, proof_response.clone()).await;
                        continue;
                    }
                };

                let try_generate_proof = prove(Backend::Exec, program_input, ctx.aligned_mode)
                    .and_then(|output| to_batch_proof(output, ctx.aligned_mode))
                    .map_err(|err| {
                        RpcErr::Internal(format!("Error while generate proof: {:}", err))
                    });

                let (batch_proof, error) = match try_generate_proof {
                    Ok(proof) => (Some(proof), None),
                    Err(e) => (None, Some(e.to_string())),
                };

                let proof_response = ProofResponse {
                    job_id: job_id.clone(),
                    batch_number,
                    error,
                    batch_proof,
                };

                ctx.upsert_proof(&job_id, proof_response.clone()).await;
                match client
                    .send_proof_response(&proof_response, &job.sequencer_url)
                    .await{
                        Ok(_) => {
                            info!("");
                        }
                        Err(err) => {
                            tracing::error!("Proof sending error: {:}", err.to_string());
                        }
                    }
            }
            None => {
                continue;
            }
        }
    }
}
