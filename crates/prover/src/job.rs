use mojave_client::types::{JobId, ProofResponse};
use reqwest::Url;
use std::collections::{HashMap, HashSet};
use tokio::sync::Mutex;

pub struct JobRecord {
    pub job_id: JobId,
    pub prover_data: mojave_client::types::ProverData,
    pub sequencer_url: Url,
}

pub struct JobStore {
    pending: Mutex<HashSet<JobId>>,
    proofs: Mutex<HashMap<JobId, ProofResponse>>,
}

impl Default for JobStore {
    fn default() -> Self {
        JobStore {
            pending: Mutex::new(HashSet::new()),
            proofs: Mutex::new(HashMap::new()),
        }
    }
}

impl JobStore {
    pub async fn already_requested(&self, job_id: &JobId) -> bool {
        if self.pending.lock().await.contains(job_id) {
            true
        } else {
            self.proofs.lock().await.contains_key(job_id)
        }
    }

    pub async fn get_pending_jobs(&self) -> Vec<JobId> {
        let g = self.pending.lock().await;
        g.iter().cloned().collect()
    }

    pub async fn insert_job(&self, job_id: JobId) {
        self.pending.lock().await.insert(job_id.to_owned());
    }

    pub async fn get_proof_by_id(&self, job_id: &JobId) -> Option<ProofResponse> {
        self.proofs.lock().await.get(job_id).cloned()
    }

    pub async fn upsert_proof(&self, job_id: &JobId, proof_response: ProofResponse) {
        self.pending.lock().await.remove(job_id);
        self.proofs
            .lock()
            .await
            .insert(job_id.to_owned(), proof_response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mojave_client::types::{ProofResponse, ProofResult};

    fn mk_proof(job_id: JobId) -> ProofResponse {
        ProofResponse {
            job_id,
            batch_number: 1,
            result: ProofResult::Error("dummy".into()),
        }
    }

    #[tokio::test]
    async fn already_requested_checks_pending_then_proofs() {
        let store = JobStore::default();

        store.insert_job("aa".into()).await;
        assert!(store.already_requested(&"aa".into()).await);

        // if it’s in proofs set, also returns true
        store.upsert_proof(&"bb".into(), mk_proof("b".into())).await;
        assert!(store.already_requested(&"bb".into()).await);
    }

    #[tokio::test]
    async fn insert_and_get_pending_jobs() {
        let store = JobStore::default();
        store.insert_job("abbaa12".into()).await;
        store.insert_job("baa2b1b".into()).await;
        store.insert_job("cac3c3c".into()).await;

        let mut got = store.get_pending_jobs().await;
        got.sort();
        assert_eq!(
            got,
            vec!["abbaa12".into(), "baa2b1b".into(), "cac3c3c".into()]
        );
    }

    #[tokio::test]
    async fn upsert_proof_moves_from_pending_to_proofs() {
        let store = JobStore::default();
        store.insert_job("job-1".into()).await;
        store
            .upsert_proof(&"job-1".into(), mk_proof("job-1".into()))
            .await;

        // removed from pending
        let mut pending = store.get_pending_jobs().await;
        pending.sort();
        assert!(pending.is_empty());

        // available in proofs
        let p = store
            .get_proof_by_id(&"job-1".into())
            .await
            .expect("proof exists");
        assert_eq!(p.job_id, "job-1".into());
    }

    #[tokio::test]
    async fn get_proof_by_id_none_when_absent() {
        let store = JobStore::default();
        assert!(store.get_proof_by_id(&"missing".into()).await.is_none());
    }
}
