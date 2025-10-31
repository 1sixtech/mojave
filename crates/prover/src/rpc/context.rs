use std::{collections::HashSet, sync::Arc};

use mojave_msgio::types::Publisher;
use tokio::sync::{Mutex, mpsc};

use crate::{job::JobStore, rpc::types::JobRecord};

pub struct ProverRpcContext {
    pub aligned_mode: bool,
    pub job_store: JobStore,
    pub sender: mpsc::Sender<JobRecord>,
    pub publisher: Arc<dyn Publisher>,
    pub sent_ids: Mutex<HashSet<String>>,
}
