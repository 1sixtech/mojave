use mojave_client::types::{ProofResponse, ProverData};
use reqwest::Url;
use std::collections::{HashMap, HashSet};
use tokio::sync::{Mutex, mpsc};

pub struct ProverRpcContext {
    pub aligned_mode: bool,
    pub job_store: JobStore,
    pub sender: mpsc::Sender<JobRecord>,
}

pub struct SendProofInputRequest {
    pub prover_data: ProverData,
    pub sequencer_addr: Url,
}

pub struct JobRecord {
    pub job_id: String,
    pub prover_data: ProverData,
    pub sequencer_url: Url,
}

pub struct JobStore {
    pending: Mutex<HashSet<String>>,
    proofs: Mutex<HashMap<String, ProofResponse>>,
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
    pub async fn already_requested(&self, job_id: &str) -> bool {
        if self.pending.lock().await.contains(job_id) {
            true
        } else {
            self.proofs.lock().await.contains_key(job_id)
        }
    }

    pub async fn get_pending_jobs(&self) -> Vec<String> {
        let g = self.pending.lock().await;
        g.iter().cloned().collect()
    }

    pub async fn insert_job(&self, job_id: &str) {
        self.pending.lock().await.insert(job_id.to_owned());
    }

    pub async fn get_proof_by_id(&self, job_id: &str) -> Option<ProofResponse> {
        self.proofs.lock().await.get(job_id).cloned()
    }

    pub async fn upsert_proof(&self, job_id: &str, proof_response: ProofResponse) {
        self.pending.lock().await.remove(job_id);
        self.proofs
            .lock()
            .await
            .insert(job_id.to_owned(), proof_response);
    }
}
