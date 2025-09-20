use tokio::sync::mpsc;

use crate::{error::Result, types::BatchEvents};

pub struct Notifier {
    tx: mpsc::Sender<u64>,
}

impl Notifier {
    pub fn new(tx: mpsc::Sender<u64>) -> Self {
        Notifier { tx }
    }
}

impl BatchEvents for Notifier {
    fn on_batch_committed(&self, batch_id: u64) -> Result<()> {
        if let Err(e) = self.tx.try_send(batch_id) {
            tracing::warn!(error = ?e, "Failed to send batch ID");
            return Err(e.into());
        }

        Ok(())
    }
}
