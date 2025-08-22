use std::sync::Arc;

use reqwest::Url;
use serde_json::{Value, json};
use tiny_keccak::{Hasher, Keccak};
use tracing::info;

use ethrex_rpc::{RpcErr, utils::RpcRequest};
use zkvm_interface::io::ProgramInput;

use mojave_chain_utils::prover_types::ProverData;

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
                RpcErr::BadParams(format!("Can't parse 1st param as ProverData: {err}"))
            })?;
        let sequencer_addr = serde_json::from_value::<Url>(params[1].clone())
            .map_err(|err| RpcErr::BadParams(format!("Can't parse 2nd param as Url: {err}")))?;

        Ok(SendProofInputRequest {
            prover_data,
            sequencer_addr,
        })
    }

    pub async fn call(req: &RpcRequest, ctx: Arc<ProverRpcContext>) -> Result<Value, RpcErr> {
        let proof_input = Self::get_proof_input(&req.params)?;

        let job_id = Self::calculate_job_id(&proof_input.prover_data.input)?;
        tracing::debug!(%job_id, sequencer = %proof_input.sequencer_addr, "Parsed proof input");
        if ctx.job_store.already_requested(&job_id).await {
            tracing::warn!(%job_id, "Duplicate batch requested");
            return Err(RpcErr::BadParams("This batch already requested".to_owned()));
        }

        let record = JobRecord {
            job_id: job_id.clone(),
            prover_data: proof_input.prover_data,
            sequencer_url: proof_input.sequencer_addr.clone(),
        };

        ctx.job_store.insert_job(&record.job_id).await;

        match ctx.sender.send(record).await {
            Ok(()) => {
                tracing::info!("Job inserted into channel");
            }
            Err(err) => {
                let msg = format!("Error sending job to channel: {err}");
                tracing::error!("{}", &msg);
                return Err(RpcErr::Internal(msg));
            }
        }

        info!(%job_id, "Job enqueued");

        Ok(json!(job_id))
    }

    fn calculate_job_id(prover_input: &ProgramInput) -> Result<String, RpcErr> {
        let mut block_hashes: Vec<String> = prover_input
            .blocks
            .iter()
            .map(|b| b.hash().to_string())
            .collect();
        // GW: Should I rm sort?
        block_hashes.sort_unstable();
        let serialized_block_hashest = bincode::serialize(&block_hashes)
            .map_err(|err| RpcErr::Internal(format!("Error to serialize program input: {err}")))?;

        let mut hasher = Keccak::v256();
        hasher.update(&serialized_block_hashest);
        let mut hash = [0_u8; 32];
        hasher.finalize(&mut hash);

        tracing::trace!(job_id = %hex::encode(hash), "Calculated job_id");
        Ok(hex::encode(hash))
    }
}

pub struct GetJobIdRequest;
impl GetJobIdRequest {
    pub async fn call(req: &RpcRequest, ctx: Arc<ProverRpcContext>) -> Result<Value, RpcErr> {
        if let Some(param) = req.params.as_ref() {
            tracing::warn!(got = param.len(), "mojave_getJobID expects no params");
            return Err(RpcErr::BadParams(format!(
                "Expected 0 params, got {}",
                param.len()
            )));
        };
        let pending = ctx.job_store.get_pending_jobs().await;
        tracing::debug!(count = pending.len(), "Returning pending jobs");
        Ok(json!(pending))
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
            .job_store
            .get_proof_by_id(&job_id)
            .await
            .ok_or(RpcErr::Internal(format!(
                "No proof exist with job id {}",
                &job_id
            )))?;
        tracing::info!(job_id = %job_id, "Returning proof for job_id");
        Ok(json!(proof))
    }
}
