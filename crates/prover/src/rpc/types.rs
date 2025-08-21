use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use reqwest::Url;
use tokio::sync::{Mutex, mpsc};

use ethrex_rpc::{RpcErr, utils::RpcRequest};

use mojave_chain_utils::prover_types::{ProofResponse, ProverData};

#[derive(Clone)]
pub struct JobRecord {
    pub job_id: String,
    pub prover_data: Arc<ProverData>,
    pub sequencer_url: Url,
}

pub enum RpcNamespace {
    Mojave,
}

impl RpcNamespace {
    pub fn resolve_namespace(request: &RpcRequest) -> Result<Self, RpcErr> {
        let mut parts = request.method.split('_');
        let Some(namespace) = parts.next() else {
            return Err(RpcErr::MethodNotFound(request.method.clone()));
        };
        match namespace {
            "mojave" => Ok(Self::Mojave),
            _others => Err(RpcErr::MethodNotFound(request.method.to_owned())),
        }
    }
}

pub struct ProverRpcContext {
    aligned_mode: bool,
    job_store: JobStore,
    sender: mpsc::Sender<JobRecord>,
}

impl ProverRpcContext {
    pub fn new(aligned_mode: bool, sender: mpsc::Sender<JobRecord>) -> Self {
        ProverRpcContext {
            aligned_mode,
            job_store: JobStore::default(),
            sender,
        }
    }

    pub fn aligned_mode(&self) -> bool{
        self.aligned_mode
    }

    pub async fn already_requested(&self, job_id: &str) -> bool {
        self.job_store.already_requested(job_id).await
    }

    pub async fn get_pending_jobs(&self) -> Vec<String> {
        self.job_store.get_pending_jobs().await
    }

    pub async fn get_proof_by_id(&self, job_id: &str) -> Option<ProofResponse> {
        self.job_store.get_proof_by_id(job_id).await
    }

    pub async fn upsert_proof(&self, job_id: &str, proof_response: ProofResponse) {
        self.job_store.upsert_proof(job_id, proof_response).await;
    }

    pub async fn insert_job_sender(&self, job_record: JobRecord) -> Result<(), RpcErr> {
        self.job_store.insert_job(&job_record.job_id).await;

        match self.sender.send(job_record).await {
            Ok(()) => {
                tracing::info!("Job inserted into channel");
                Ok(())
            }
            Err(err) => {
                let msg = format!("Error sending job to channel: {:}", err);
                tracing::error!("{}", &msg);
                Err(RpcErr::Internal(msg))
            }
        }
    }
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
