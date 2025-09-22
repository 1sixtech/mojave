use mojave_client::types::JobId;

use crate::{
    rpc::{ProverRpcContext, types::SendProofInputParam},
    services::jobs::{
        enqueue_proof_input, get_pending_job_ids as jobs_get_pending_job_ids,
        get_proof as get_proof_by_id,
    },
};
use std::sync::Arc;

#[mojave_rpc_macros::rpc(namespace = "moj", method = "getPendingJobIds")]
pub async fn get_pending_job_ids(
    ctx: Arc<ProverRpcContext>,
    _params: (),
) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
    let job_ids = jobs_get_pending_job_ids(&ctx).await?;
    let job_ids = serde_json::to_value(job_ids)
        .map_err(|e| mojave_rpc_core::RpcErr::Internal(e.to_string()))?;
    Ok(job_ids)
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
    job_id: JobId,
) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
    let proof = get_proof_by_id(&ctx, &job_id).await?;
    let proof = serde_json::to_value(proof)
        .map_err(|e| mojave_rpc_core::RpcErr::Internal(e.to_string()))?;
    Ok(proof)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        job::{JobRecord, JobStore},
        rpc::{ProverRpcContext, types::SendProofInputRequest},
    };
    use guest_program::input::ProgramInput;
    use mojave_client::types::{ProofResponse, ProofResult, ProverData};
    use reqwest::Url;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    fn dummy_prover_data() -> ProverData {
        ProverData {
            batch_number: 0,
            input: ProgramInput::default(),
        }
    }

    fn make_ctx(capacity: usize) -> (Arc<ProverRpcContext>, mpsc::Receiver<JobRecord>) {
        let (tx, rx) = mpsc::channel::<JobRecord>(capacity);
        let ctx = Arc::new(ProverRpcContext {
            aligned_mode: false,
            job_store: JobStore::default(),
            sender: tx,
        });
        (ctx, rx)
    }

    #[tokio::test]
    async fn send_proof_input_accepts_tuple_and_emits_record() {
        let (ctx, mut rx) = make_ctx(8);

        super::send_proof_input(
            ctx.clone(),
            SendProofInputParam::Tuple((
                dummy_prover_data(),
                Url::parse("http://localhost:1234").unwrap(),
            )),
        )
        .await
        .unwrap();

        let rec = rx.recv().await.expect("record sent");

        assert_eq!(rec.sequencer_url.as_str(), "http://localhost:1234/");
        assert!(!rec.job_id.is_empty());
    }

    #[tokio::test]
    async fn send_proof_input_accepts_object_and_emits_record() {
        let (ctx, mut rx) = make_ctx(8);

        super::send_proof_input(
            ctx.clone(),
            SendProofInputParam::Object(SendProofInputRequest {
                prover_data: dummy_prover_data(),
                sequencer_addr: Url::parse("http://localhost:4321").unwrap(),
            }),
        )
        .await
        .unwrap();

        let rec = rx.recv().await.expect("record sent");

        assert_eq!(rec.sequencer_url.as_str(), "http://localhost:4321/");
        assert!(!rec.job_id.is_empty());
    }

    #[tokio::test]
    async fn send_proof_input_rejects_duplicate_job_in_same_context() {
        let (ctx, mut _rx) = make_ctx(8);

        super::send_proof_input(
            ctx.clone(),
            SendProofInputParam::Tuple((
                dummy_prover_data(),
                Url::parse("http://localhost:1234").unwrap(),
            )),
        )
        .await
        .unwrap();

        let err = super::send_proof_input(
            ctx.clone(),
            SendProofInputParam::Tuple((
                dummy_prover_data(),
                Url::parse("http://localhost:1234").unwrap(),
            )),
        )
        .await
        .unwrap_err();

        let s = format!("{err:#}");
        assert!(s.to_lowercase().contains("already requested"));
    }

    #[tokio::test]
    async fn get_pending_job_ids_returns_json_array_of_ids() {
        let (ctx, _rx) = make_ctx(1);
        ctx.job_store.insert_job("abbaa12".into()).await;
        ctx.job_store.insert_job("baa2b1b".into()).await;
        ctx.job_store.insert_job("cac3c3c".into()).await;

        let val = super::get_pending_job_ids(ctx, ()).await.unwrap();

        let mut arr = val.as_array().unwrap().clone();
        assert_eq!(arr.len(), 3);

        arr.sort_by(|x, y| x.as_str().cmp(&y.as_str()));
        let got: Vec<&str> = arr.iter().map(|v| v.as_str().unwrap()).collect();

        assert_eq!(got, vec!["abbaa12", "baa2b1b", "cac3c3c"]);
    }

    #[tokio::test]
    async fn get_proof_serializes_proof_to_json() {
        let (ctx, _rx) = make_ctx(1);
        let expected = ProofResponse {
            job_id: "job-1".into(),
            batch_number: 7,
            result: ProofResult::Error("dummy".to_string()),
        };
        ctx.job_store
            .upsert_proof(&"job-1".into(), expected.clone())
            .await;

        let val = super::get_proof(ctx, "job-1".into()).await.unwrap();

        assert_eq!(val, serde_json::to_value(&expected).unwrap());
    }
}
