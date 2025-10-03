use ethrex_common::types::batch::Batch;

use crate::{error::Result, types::BatchQueue};

/// No-op implementation of BatchQueue that does nothing with batches.
/// This is useful as a default implementation when no queue service is configured.
pub struct NoOpBatchQueue;

impl NoOpBatchQueue {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoOpBatchQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl BatchQueue for NoOpBatchQueue {
    fn send_batch(&self, batch: &Batch) -> Result<()> {
        tracing::debug!(
            batch_number = batch.number,
            first_block = batch.first_block,
            last_block = batch.last_block,
            "NoOpBatchQueue: batch would be sent to queue"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethrex_common::{H256, types::BlobsBundle};

    #[test]
    fn test_noop_batch_queue() {
        let queue = NoOpBatchQueue::new();
        let batch = Batch {
            number: 1,
            first_block: 0,
            last_block: 10,
            state_root: H256::default(),
            privileged_transactions_hash: H256::default(),
            message_hashes: vec![],
            blobs_bundle: BlobsBundle::default(),
            commit_tx: None,
            verify_tx: None,
        };

        assert!(queue.send_batch(&batch).is_ok());
    }

    #[test]
    fn test_noop_batch_queue_default() {
        let queue = NoOpBatchQueue::default();
        let batch = Batch {
            number: 2,
            first_block: 11,
            last_block: 20,
            state_root: H256::default(),
            privileged_transactions_hash: H256::default(),
            message_hashes: vec![],
            blobs_bundle: BlobsBundle::default(),
            commit_tx: None,
            verify_tx: None,
        };

        assert!(queue.send_batch(&batch).is_ok());
    }
}
