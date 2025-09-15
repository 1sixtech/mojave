use crate::{error::Result, types::BatchEvents};

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

    #[allow(dead_code)]
    fn commit_next_batch_to_l1(&self, batch_id: u64) -> Result<()> {
        // TODO: Implement the logic to commit the next batch to L1
        self.notifier.on_batch_committed(batch_id)?;
        Ok(())
    }
}
