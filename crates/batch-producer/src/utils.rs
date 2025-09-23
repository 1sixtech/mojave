use crate::{
    BatchProducerContext,
    error::{Error, Result},
};

/// Get the last block number part of the given batch number
pub(crate) async fn get_last_committed_block(
    ctx: &BatchProducerContext,
    batch_number: u64,
) -> Result<u64> {
    let last_committed_blocks = ctx
           .rollup_store
           .get_block_numbers_by_batch(batch_number)
           .await?
           .ok_or_else(|| {
               Error::RetrievalError(format!(
                   "Failed to get batch with batch number {batch_number}. Batch is missing when it should be present. This is a bug",
               ))
           })?;

    let last_committed_block = last_committed_blocks.last().ok_or_else(|| {
        Error::RetrievalError(format!(
            "Last committed batch ({batch_number}) doesn't have any blocks. This is probably a bug.",
        ))
    })?;

    Ok(*last_committed_block)
}
