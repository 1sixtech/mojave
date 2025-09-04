use tokio::sync::mpsc;

use crate::{job::JobStore, rpc::types::JobRecord};

pub struct ProverRpcContext {
    pub aligned_mode: bool,
    pub job_store: JobStore,
    pub sender: mpsc::Sender<JobRecord>,
}
