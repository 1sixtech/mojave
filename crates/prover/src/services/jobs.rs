use crate::{job::JobRecord, rpc::ProverRpcContext};
use guest_program::input::ProgramInput;
use mojave_client::types::{ProofResponse, ProverData};
use mojave_utils::rpc::error::{Error, Result};
use reqwest::Url;
use tiny_keccak::{Hasher, Keccak};

pub async fn enqueue_proof_input(
    ctx: &ProverRpcContext,
    prover_data: ProverData,
    sequencer_addr: Url,
) -> Result<String> {
    let job_id = calculate_job_id(&prover_data.input)?;
    tracing::debug!(%job_id, sequencer = %sequencer_addr, "Parsed proof input");
    if ctx.job_store.already_requested(&job_id).await {
        tracing::warn!(%job_id, "Duplicate batch requested");
        return Err(Error::BadParams("This batch already requested".to_owned()));
    }

    let record = JobRecord {
        job_id: job_id.clone(),
        prover_data,
        sequencer_url: sequencer_addr,
    };
    ctx.job_store.insert_job(&record.job_id).await;
    ctx.sender
        .send(record)
        .await
        .map_err(|e| Error::Internal(format!("Error sending job to channel: {e}")))?;
    Ok(job_id)
}

pub async fn get_pending_jobs(ctx: &ProverRpcContext) -> Result<Vec<String>> {
    Ok(ctx.job_store.get_pending_jobs().await)
}

pub async fn get_proof(ctx: &ProverRpcContext, job_id: &str) -> Result<ProofResponse> {
    ctx.job_store
        .get_proof_by_id(job_id)
        .await
        .ok_or(Error::Internal(format!(
            "No proof exist with job id {job_id}"
        )))
}

fn calculate_job_id(prover_input: &ProgramInput) -> Result<String> {
    let mut block_hashes: Vec<String> = prover_input
        .blocks
        .iter()
        .map(|b| b.hash().to_string())
        .collect();
    block_hashes.sort_unstable();
    let serialized_block_hashes = bincode::serialize(&block_hashes)
        .map_err(|err| Error::Internal(format!("Error to serialize program input: {err}")))?;

    let mut hasher = Keccak::v256();
    hasher.update(&serialized_block_hashes);
    let mut hash = [0_u8; 32];
    hasher.finalize(&mut hash);
    let job_id = hex::encode(hash);
    tracing::trace!(%job_id, "Calculated job_id");
    Ok(job_id)
}
