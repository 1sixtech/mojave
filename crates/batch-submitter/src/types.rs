use crate::error::Result;

/// Notification interface for downstream components interested in batch related events.
pub trait BatchEvents: Send + Sync {
    fn on_batch_committed(&self, batch_id: u64) -> Result<()>;
}
