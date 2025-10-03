use crate::error::Result;
use ethrex_common::types::batch::Batch;

/// Notification interface for downstream components interested in batch related events.
pub trait BatchEvents: Send + Sync {
    fn on_batch_committed(&self, batch_id: u64) -> Result<()>;
}

/// Queue interface for sending batches to a Q service.
pub trait BatchQueue: Send + Sync {
    /// Send a batch to the queue service
    fn send_batch(&self, batch: &Batch) -> Result<()>;
}
