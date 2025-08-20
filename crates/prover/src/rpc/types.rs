use std::{collections::HashMap, hash::Hash, sync::Arc};

use ethrex_l2_common::prover::BatchProof;
use ethrex_rpc::{
    RpcErr,
    utils::{RpcRequest, RpcRequestId},
};
use serde::{Deserialize, Serialize};

use tokio::sync::Mutex;
use zkvm_interface::io::ProgramInput;

#[derive(Deserialize, Serialize)]
pub struct ProverData {
    pub batch_number: u64,
    pub input: ProgramInput,
}

#[derive(Clone, Debug)]
pub struct ProofResponse {
    pub job_id: String,
    pub batch_number: u64,
    pub batch_proof: Option<BatchProof>,
}

pub enum JobStatus {
    Pending,
    Done,
    Error,
    NotExist,
}

#[derive(Clone)]
pub struct JobRecord {
    pub job_id: String,
    pub prover_data: Arc<ProverData>,
    pub sequencer_endpoint: String,
    pub error: Option<String>,
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
    pub job_queue: Mutex<HashMap<String, JobRecord>>,
    pub proofs: Mutex<HashMap<String, ProofResponse>>,
}

impl ProverRpcContext {
    pub async fn get_job_by_id(&self, job_id: &str) -> JobStatus {
        if let Some(_) = get_map(&self.job_queue, job_id).await {
            JobStatus::Pending
        } else if let Some(proof) = get_map(&self.proofs, job_id).await {
            JobStatus::Done
        } else {
            JobStatus::NotExist
        }
    }
    pub async fn get_job_queue_by_id(&self, job_id: &str) -> Option<JobRecord> {
        get_map(&self.job_queue, job_id).await
    }
    pub async fn get_proof_by_id(&self, job_id: &str) -> Option<ProofResponse> {
        get_map(&self.proofs, job_id).await
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
    K: Eq + Hash
{
    let g = map.lock().await;
    g.insert(key, value);
}
