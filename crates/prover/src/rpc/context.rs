use tokio::sync::mpsc;

use crate::rpc::types::{JobRecord, JobStore};

pub struct ProverRpcContext {
    pub aligned_mode: bool,
    pub job_store: JobStore,
    pub sender: mpsc::Sender<JobRecord>,
}
