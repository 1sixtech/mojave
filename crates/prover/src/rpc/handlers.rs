use crate::{
    rpc::{ProverRpcContext, types::SendProofInputParam},
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
    params: SendProofInputParam,
) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
    use SendProofInputParam::*;
    let (prover_data, sequencer_addr) = match params {
        Object(obj) => (obj.prover_data, obj.sequencer_addr),
        Tuple((pd, url)) => (pd, url),
    };
    let job_id = enqueue_proof_input(&ctx, prover_data, sequencer_addr)
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
    use super::*;
    use mojave_client::types::ProverData;
    use reqwest::Url;
    use zkvm_interface::io::ProgramInput;

    fn dummy_prover_data() -> ProverData {
        ProverData {
            batch_number: 0,
            input: ProgramInput::default(),
        }
    }

    #[tokio::test]
    async fn test_send_proof_input_accepts_tuple_and_object() {
        let (tx, mut _rx) = tokio::sync::mpsc::channel(8);
        let ctx = std::sync::Arc::new(crate::rpc::ProverRpcContext {
            aligned_mode: false,
            job_store: crate::job::JobStore::default(),
            sender: tx,
        });

        // Tuple params form via direct handler call
        let _ = super::send_proof_input(
            ctx.clone(),
            SendProofInputParam::Tuple((
                dummy_prover_data(),
                Url::parse("http://localhost:1234").unwrap(),
            )),
        )
        .await
        .unwrap();
        // Object params form via direct handler call with a fresh context to avoid duplicate-job error
        let (tx2, mut _rx2) = tokio::sync::mpsc::channel(8);
        let ctx2 = std::sync::Arc::new(crate::rpc::ProverRpcContext {
            aligned_mode: false,
            job_store: crate::job::JobStore::default(),
            sender: tx2,
        });
        let _ = super::send_proof_input(
            ctx2.clone(),
            SendProofInputParam::Object(crate::rpc::types::SendProofInputRequest {
                prover_data: dummy_prover_data(),
                sequencer_addr: Url::parse("http://localhost:1234").unwrap(),
            }),
        )
        .await
        .unwrap();
    }
}
