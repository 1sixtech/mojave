#[derive(Debug, Clone)]
pub enum Request {
    /// Start listening for batches (non-blocking)
    StartListening,
    /// Get current status of the committer
    GetStatus,
    /// Force process a specific batch
    ProcessBatch(ethrex_common::types::batch::Batch),
    /// Get metrics about processed batches
    GetMetrics,
}

#[derive(Debug, Clone)]
pub struct CommitterStatus {
    pub is_listening: bool,
    pub batches_processed: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CommitterMetrics {
    pub total_batches_processed: u64,
    pub batches_committed_to_l1: u64,
    pub batches_published: u64,
    pub batches_broadcasted: u64,
    pub errors_count: u64,
}

#[derive(Debug)]
pub enum CommitterResponse {
    ListeningStarted { processed_batches: u32 },
    Status(CommitterStatus),
    BatchProcessed,
    Metrics(CommitterMetrics),
}
