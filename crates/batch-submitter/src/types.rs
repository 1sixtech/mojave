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
