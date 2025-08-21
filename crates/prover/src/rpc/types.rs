use reqwest::Url;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::Arc,
};
use tokio::sync::{Mutex, mpsc};

use ethrex_rpc::{RpcErr, utils::RpcRequest};

use mojave_chain_utils::prover_types::{ProofResponse, ProverData};

#[derive(Clone)]
pub enum JobStatus {
    Pending,
    Done,
    Error,
}

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
    pub aligned_mode: bool,
    pub job_status: Mutex<HashMap<String, JobStatus>>,
    pub pending_jobs: Mutex<HashSet<String>>,
    pub proofs: Mutex<HashMap<String, ProofResponse>>,
    pub sender: mpsc::Sender<JobRecord>,
}

impl ProverRpcContext {
    pub async fn get_job_status(&self, job_id: &str) -> Option<JobStatus> {
        get_map(&self.job_status, job_id).await
    }

    pub async fn upsert_job_status(&self, job_id: &str, job_status: JobStatus) {
        match job_status {
            JobStatus::Pending => {
                let mut g = self.pending_jobs.lock().await;
                g.insert(job_id.to_owned());
            }
            _ => {
                let mut g = self.pending_jobs.lock().await;
                g.remove(job_id);
            }
        }

        upsert_map(&self.job_status, job_id.to_owned(), job_status).await;
    }

    pub async fn get_pending_jobs(&self) -> Vec<String> {
        let g = self.pending_jobs.lock().await;
        g.iter().cloned().collect()
    }

    pub async fn get_proof_by_id(&self, job_id: &str) -> Option<ProofResponse> {
        get_map(&self.proofs, job_id).await
    }

    pub async fn upsert_proof(&self, job_id: &str, proof_response: ProofResponse) {
        match proof_response.error {
            Some(_) => self.upsert_job_status(job_id, JobStatus::Error).await,
            None => self.upsert_job_status(job_id, JobStatus::Done).await,
        };

        upsert_map(&self.proofs, job_id.to_owned(), proof_response).await;
    }
    

    pub async fn insert_job_sender(&self, job_record: JobRecord) -> Result<(), RpcErr> {
        self.upsert_job_status(&job_record.job_id, JobStatus::Pending).await;
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

async fn get_map<K, V, Q>(map: &Mutex<HashMap<K, V>>, key: &Q) -> Option<V>
where
    K: Eq + Hash + std::borrow::Borrow<Q>,
    Q: Eq + Hash + ?Sized,
    V: Clone,
{
    let g = map.lock().await;
    g.get(key).cloned()
}

async fn upsert_map<K, V>(map: &Mutex<HashMap<K, V>>, key: K, value: V)
where
    K: Eq + Hash,
{
    let mut g = map.lock().await;
    g.insert(key, value);
}
