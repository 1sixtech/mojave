use std::sync::Arc;

use ethrex_l2_common::prover::BatchProof;
use ethrex_rpc::{utils::RpcRequest, RpcErr};
use serde_json::Value;
use tiny_keccak::{Hasher, Keccak};
use zkvm_interface::io::ProgramInput;

use super::types::{JobRecord, ProverRpcContext, ProofResponse};
use crate::types::ProverData;

// Compute proof in-process using the same library the TCP server uses
use ethrex_prover_lib::{backends::Backend, prove, to_batch_proof};

pub struct SendProofInputRequest {
    prover_data: ProverData,
    sequencer_addr: String,
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
            serde_json::from_value::<String>(params[1].clone()).map_err(|err| {
                RpcErr::BadParams(format!("Can't parse 2nd param as ProverData: {}", err))
            })?;

        Ok(SendProofInputRequest {
            prover_data,
            sequencer_addr,
        })
    }

    // TODOs: 1. spwan worker who will generate proof and send back to sequencer
    //        2. generate job id (using keccak hash) and send it back instantly.
    //        3. check job id is already exist or not to prevent spamming
    pub async fn call(req: &RpcRequest, ctx: Arc<ProverRpcContext>) -> Result<Value, RpcErr> {
        let proof_input = Self::get_proof_input(&req.params)?;

        let job_id = Self::calculate_job_id(&proof_input.prover_data.input)?;

        // Reject duplicates
        if ctx.get_job_by_id(&job_id).await != super::types::JobStatus::NotExist {
            return Err(RpcErr::BadParams(
                "This Program input already exists in job queue or has completed".to_owned(),
            ));
        }

        // Insert into the job queue
        let record = JobRecord {
            job_id: job_id.clone(),
            prover_data: Arc::new(proof_input.prover_data.clone()),
            sequencer_endpoint: proof_input.sequencer_addr.clone(),
            error: None,
        };
        {
            let mut g = ctx.job_queue.lock().await;
            g.insert(job_id.clone(), record);
        }

        // Spawn worker to compute proof and store result
        let ctx_clone = ctx.clone();
        let job_id_clone = job_id.clone();
        let aligned = ctx.aligned_mode;
        tokio::spawn(async move {
            // Load record
            let maybe_rec = ctx_clone.get_job_queue_by_id(&job_id_clone).await;
            if let Some(rec) = maybe_rec {
                let batch_number = rec.prover_data.batch_number;
                let prover_input = rec.prover_data.input.clone();

                let result: Result<BatchProof, String> = prove(Backend::Exec, prover_input, aligned)
                    .and_then(|output| to_batch_proof(output, aligned))
                    .map_err(|e| e.to_string());

                match result {
                    Ok(batch_proof) => {
                        // Remove from queue, add to proofs
                        {
                            let mut g = ctx_clone.job_queue.lock().await;
                            g.remove(&job_id_clone);
                        }

                        let proof_response = ProofResponse {
                            job_id: job_id_clone.clone(),
                            batch_number,
                            batch_proof: Some(batch_proof),
                        };
                        let mut g = ctx_clone.proofs.lock().await;
                        g.insert(job_id_clone.clone(), proof_response);
                    }
                    Err(err) => {
                        // Remove from queue and drop; optionally log error
                        {
                            let mut g = ctx_clone.job_queue.lock().await;
                            g.remove(&job_id_clone);
                        }
                        tracing::error!(job_id = %job_id_clone, error = %err, "Proof generation failed");
                    }
                }
            }
        });

        Ok(serde_json::json!({ "job_id": job_id }))
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
    pub async fn call(req: &RpcRequest, _ctx: Arc<ProverRpcContext>) -> Result<Value, RpcErr> {
        let params = req
            .params
            .as_ref()
            .ok_or(RpcErr::BadParams("No param provided".to_owned()))?;
        if params.len() != 1 {
            return Err(RpcErr::BadParams(format!(
                "Expected 1 param, got {}",
                params.len()
            )));
        }

        // Accept either ProverData or ProgramInput directly
        let job_id = if let Ok(prover_data) = serde_json::from_value::<ProverData>(params[0].clone())
        {
            Self::calculate_job_id(&prover_data.input)?
        } else {
            let input: ProgramInput = serde_json::from_value(params[0].clone()).map_err(|err| {
                RpcErr::BadParams(format!(
                    "Can't parse param as ProverData or ProgramInput: {}",
                    err
                ))
            })?;
            Self::calculate_job_id(&input)?
        };

        Ok(serde_json::json!({ "job_id": job_id }))
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

pub struct GetProofRequest;

impl GetProofRequest {
    pub async fn call(req: &RpcRequest, ctx: Arc<ProverRpcContext>) -> Result<Value, RpcErr> {
        let params = req
            .params
            .as_ref()
            .ok_or(RpcErr::BadParams("No param provided".to_owned()))?;
        if params.len() != 1 {
            return Err(RpcErr::BadParams(format!(
                "Expected 1 param (job_id), got {}",
                params.len()
            )));
        }
        let job_id: String = serde_json::from_value(params[0].clone()).map_err(|err| {
            RpcErr::BadParams(format!("Can't parse param as job_id string: {}", err))
        })?;

        match ctx.get_job_by_id(&job_id).await {
            super::types::JobStatus::Pending => Ok(serde_json::json!({
                "job_id": job_id,
                "status": "pending"
            })),
            super::types::JobStatus::Done => {
                if let Some(proof) = ctx.get_proof_by_id(&job_id).await {
                    let value = serde_json::to_value(&proof.batch_proof)
                        .map_err(|e| RpcErr::Internal(e.to_string()))?;
                    Ok(serde_json::json!({
                        "job_id": proof.job_id,
                        "batch_number": proof.batch_number,
                        "status": "done",
                        "proof": value,
                    }))
                } else {
                    Err(RpcErr::Internal("Proof status inconsistent".to_owned()))
                }
            }
            super::types::JobStatus::Error => Ok(serde_json::json!({
                "job_id": job_id,
                "status": "error"
            })),
            super::types::JobStatus::NotExist => Err(RpcErr::BadParams("job_id not found".to_owned())),
        }
    }
}
