use tokio::sync::mpsc;

use crate::{job::JobStore, notifier::Notifier, rpc::types::JobRecord};

pub struct ProverRpcContext {
    pub aligned_mode: bool,
    pub job_store: JobStore,
    pub job_sender: mpsc::Sender<JobRecord>,
    pub notifier: Notifier,
}
