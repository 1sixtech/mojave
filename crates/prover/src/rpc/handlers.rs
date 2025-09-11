use mojave_client::types::ProverData;

use crate::{
    rpc::ProverRpcContext,
    services::jobs::{enqueue_proof_input, get_pending_jobs, get_proof as get_proof_by_id},
};
use std::sync::Arc;

#[mojave_rpc_macros::rpc(namespace = "moj", method = "getJobId")]
pub async fn get_job_id(
    ctx: Arc<ProverRpcContext>,
    _params: (),
) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
    let pending = get_pending_jobs(&ctx).await?;
    Ok(serde_json::to_value(pending).unwrap())
}

#[mojave_rpc_macros::rpc(namespace = "moj", method = "sendProofInput")]
pub async fn send_proof_input(
    ctx: Arc<ProverRpcContext>,
    params: ProverData,
) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
    let job_id = enqueue_proof_input(&ctx, params)
        .await
        .map_err(|e| mojave_rpc_core::RpcErr::Internal(e.to_string()))?;
    Ok(serde_json::json!(job_id))
}

#[mojave_rpc_macros::rpc(namespace = "moj", method = "getProof")]
pub async fn get_proof(
    ctx: Arc<ProverRpcContext>,
    job_id: String,
) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
    let proof = get_proof_by_id(&ctx, &job_id).await?;
    Ok(serde_json::to_value(proof).unwrap())
}

#[cfg(test)]
mod tests {
    use guest_program::input::ProgramInput;
    use mojave_client::types::ProverData;

    fn dummy_prover_data() -> ProverData {
        ProverData {
            batch_number: 0,
            input: ProgramInput::default(),
        }
    }

    #[tokio::test]
    async fn test_send_proof_input() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let ctx = std::sync::Arc::new(crate::rpc::ProverRpcContext {
            aligned_mode: false,
            job_store: crate::job::JobStore::default(),
            sender: tx,
        });

        // Call send_proof_input and get the job_id result
        let result = super::send_proof_input(ctx.clone(), dummy_prover_data())
            .await
            .unwrap();

        let job_id: String = serde_json::from_value(result).expect("Should be a string job_id");

        // check that the job_id is in the pending jobs
        let pending_jobs = crate::services::jobs::get_pending_jobs(&ctx).await.unwrap();
        assert!(pending_jobs.contains(&job_id));

        // check that the job was sent to the channel
        let job_record = rx.recv().await.expect("Should receive a job record");
        assert_eq!(job_record.job_id, job_id);
    }
}
