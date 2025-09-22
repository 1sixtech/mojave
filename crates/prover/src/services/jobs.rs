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

#[inline]
pub async fn get_pending_job_ids(ctx: &ProverRpcContext) -> Result<Vec<String>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        job::{JobRecord, JobStore},
        rpc::ProverRpcContext,
    };
    use guest_program::input::ProgramInput;
    use mojave_client::types::{ProofResponse, ProofResult, ProverData};
    use tokio::sync::mpsc;

    fn dummy_data() -> ProverData {
        ProverData {
            batch_number: 0,
            input: ProgramInput::default(),
        }
    }

    fn make_ctx(cap: usize) -> (ProverRpcContext, mpsc::Receiver<JobRecord>) {
        let (tx, rx) = mpsc::channel::<JobRecord>(cap);
        (
            ProverRpcContext {
                aligned_mode: false,
                job_store: JobStore::default(),
                sender: tx,
            },
            rx,
        )
    }

    #[tokio::test]
    async fn enqueue_proof_input_enqueues_and_returns_job_id() {
        let (ctx, mut rx) = make_ctx(8);
        let url = Url::parse("http://localhost:1234").unwrap();

        let job_id = enqueue_proof_input(&ctx, dummy_data(), url.clone())
            .await
            .unwrap();

        let rec = rx.recv().await.unwrap();
        assert_eq!(rec.job_id, job_id);
        assert_eq!(rec.sequencer_url, url);

        let mut list = ctx.job_store.get_pending_jobs().await;
        assert_eq!(list.pop().unwrap(), job_id);
    }

    #[tokio::test]
    async fn enqueue_proof_input_rejects_duplicate() {
        let (ctx, _rx) = make_ctx(8);
        let url = Url::parse("http://localhost:1234").unwrap();

        enqueue_proof_input(&ctx, dummy_data(), url.clone())
            .await
            .unwrap();
        let err = enqueue_proof_input(&ctx, dummy_data(), url)
            .await
            .unwrap_err();

        let s = format!("{err:?}").to_lowercase();
        assert!(s.contains("already requested"));
    }

    #[tokio::test]
    async fn get_proof_returns_existing_or_err() {
        let (ctx, _rx) = make_ctx(8);

        let expected = ProofResponse {
            job_id: "job-1".into(),
            batch_number: 1,
            result: ProofResult::Error("dummy".into()),
        };
        ctx.job_store.upsert_proof("job-1", expected.clone()).await;

        let ok = get_proof(&ctx, "job-1").await.unwrap();
        assert_eq!(ok.job_id, expected.job_id);

        let err = get_proof(&ctx, "nope").await.unwrap_err();
        let s = format!("{err:?}").to_lowercase();
        assert!(s.contains("no proof"));
    }

    #[tokio::test]
    async fn calculate_job_id_is_stable_for_same_input() {
        let input = ProgramInput::default();
        let a = super::calculate_job_id(&input).unwrap();
        let b = super::calculate_job_id(&input).unwrap();
        assert_eq!(a, b);
    }
}
