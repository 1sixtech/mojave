use tokio::sync::mpsc;

use crate::{error::Result, notifier::Notifier, types::BatchEvents};

pub struct Committer<E>
where
    E: BatchEvents,
{
    notifier: E,
}

impl<E> Committer<E>
where
    E: BatchEvents,
{
    pub fn new(notifier: E) -> Self {
        Self { notifier }
    }

    pub async fn run(batch_tx: mpsc::Sender<u64>) -> Result<()> {
        let batch_notifier = Notifier::new(batch_tx);
        let _: Committer<Notifier> = Committer::new(batch_notifier);

        tracing::info!("Commiter started but doing nothing as of right now");

        Ok(())
    }

    #[allow(dead_code)]
    fn commit_next_batch_to_l1(&self, batch_id: u64) -> Result<()> {
        // TODO: Implement the logic to commit the next batch to L1
        self.notifier.on_batch_committed(batch_id)?;
        Ok(())
    }
}
