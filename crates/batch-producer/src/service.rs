use crate::{
    context::BatchProducerContext,
    error::{Error, Result},
    utils::get_last_committed_block,
};

use ethrex_blockchain::vm::StoreVmDatabase;
use ethrex_common::{
    Address, H256,
    types::{
        AccountUpdate, BlobsBundle, Block, BlockHeader, BlockNumber, PrivilegedL2Transaction,
        batch::Batch, blobs_bundle,
    },
};
use ethrex_l2_common::{
    l1_messages::{L1Message, get_l1_message_hash},
    privileged_transactions::compute_privileged_transactions_hash,
    state_diff::StateDiff,
};
use ethrex_vm::VmDatabase;
use std::collections::{HashMap, hash_map::Entry};
use tracing::{debug, info, warn};

struct BatchData {
    last_block: BlockNumber,
    state_root: H256,
    message_hashes: Vec<H256>,
    privileged_tx_hashes: Vec<H256>,
    blobs_bundle: BlobsBundle,
}

struct BlockData {
    block: Block,
    header: BlockHeader,
}

pub struct BatchProducer {
    // TODO: replace that with a real batch counter (getting the batch counter from the context/l1)
    // dummy batch counter for the moment
    batch_counter: u64,
}

impl Default for BatchProducer {
    fn default() -> Self {
        Self::new(0)
    }
}

impl BatchProducer {
    pub fn new(batch_counter: u64) -> Self {
        BatchProducer { batch_counter }
    }

    pub async fn build_batch(&mut self, ctx: &BatchProducerContext) -> Result<Option<Batch>> {
        let batch_number = self.batch_counter + 1;

        debug!(
            last_commited_batch_number = self.batch_counter,
            batch_number, "Building batch"
        );

        // TODO: add a check if we already have the batch in the rollup_store ?

        let last_block = get_last_committed_block(ctx, self.batch_counter).await?;
        let first_block = last_block + 1;
        let batch_data = self
            .prepare_batch_from_block(ctx, last_block, first_block, batch_number)
            .await?;

        let Some(batch_data) = batch_data else {
            debug!("No new blocks to commit, skipping batch creation");
            return Ok(None);
        };

        let batch = self.create_batch(batch_number, first_block, batch_data)?;

        ctx.rollup_store.seal_batch(batch.clone()).await?;

        debug!(
            first_block = batch.first_block,
            last_block = batch.last_block,
            batch_number = batch.number,
            "Batch stored in database",
        );

        // SUCCESS update batch counter
        self.batch_counter += 1;

        // TODO add send committment

        Ok(Some(batch))
    }

    async fn create_parent_database(
        &self,
        ctx: &BatchProducerContext,
        first_block: BlockNumber,
    ) -> Result<StoreVmDatabase> {
        let parent_hash = ctx
            .store
            .get_block_header(first_block)?
            .ok_or_else(|| {
                Error::FailedToGetInformationFromStorage(format!(
                    "Failed to get block header for block {first_block}"
                ))
            })?
            .parent_hash;

        Ok(StoreVmDatabase::new(ctx.store.clone(), parent_hash))
    }

    fn get_block_state_root(&self, ctx: &BatchProducerContext, block: &Block) -> Result<H256> {
        let hash = ctx
            .store
            .state_trie(block.hash())?
            .ok_or_else(|| {
                Error::FailedToGetInformationFromStorage(
                    "Failed to get state root from storage".to_string(),
                )
            })?
            .hash_no_commit();
        Ok(hash)
    }

    fn create_batch(
        &self,
        batch_number: u64,
        first_block: BlockNumber,
        data: BatchData,
    ) -> Result<Batch> {
        let privileged_transactions_hash =
            compute_privileged_transactions_hash(data.privileged_tx_hashes)?;

        Ok(Batch {
            number: batch_number,
            first_block,
            last_block: data.last_block,
            state_root: data.state_root,
            privileged_transactions_hash,
            message_hashes: data.message_hashes,
            blobs_bundle: data.blobs_bundle,
            commit_tx: None,
            verify_tx: None,
        })
    }

    async fn get_block_data(
        &self,
        ctx: &BatchProducerContext,
        block_number: BlockNumber,
    ) -> Result<Option<BlockData>> {
        let Some(body) = ctx.store.get_block_body(block_number).await? else {
            return Ok(None);
        };

        let header = ctx
            .store
            .get_block_header(block_number)
            .map_err(Error::from)?
            .ok_or_else(|| {
                Error::FailedToGetInformationFromStorage(format!(
                    "Missing block header for block {block_number} after body was found"
                ))
            })?;

        let block = Block::new(header.clone(), body);

        Ok(Some(BlockData { block, header }))
    }

    async fn process_block(
        &self,
        ctx: &BatchProducerContext,
        block_data: &BlockData,
    ) -> Result<(
        Vec<L1Message>,
        Vec<PrivilegedL2Transaction>,
        Vec<AccountUpdate>,
    )> {
        let messages = get_block_l1_messages();
        let privileged_txs = get_privileged_transactions();
        let account_updates =
            load_or_execute_updates(ctx, &block_data.block, block_data.header.number).await?;

        Ok((messages, privileged_txs, account_updates))
    }

    async fn prepare_batch_from_block(
        &mut self,
        ctx: &BatchProducerContext,
        last_committed_block: BlockNumber,
        first_block: BlockNumber,
        batch_number: u64,
    ) -> Result<Option<BatchData>> {
        info!(first_block, batch_number, "Preparing batch");

        let parent_db = self.create_parent_database(ctx, first_block).await?;
        let mut accumulator = BatchAccumulator::default();
        let mut blobs_bundle = BlobsBundle::default();
        let mut state_root = H256::default();
        let mut current_block = first_block;

        loop {
            let block_number = current_block;

            // get body and header of current block we wish to add to the batch
            let Some(block_data) = self.get_block_data(ctx, block_number).await? else {
                debug!("No more blocks available for batch");
                break;
            };

            // TODO: add gas check

            let (messages, privileged_txs, account_updates) =
                self.process_block(ctx, &block_data).await?;

            accumulator.add_block_data(messages, privileged_txs, account_updates);

            // TODO: this is taken from ethrex let check if we need this
            // let acc_privileged_txs_len: u64 = acc_privileged_txs.len().try_into()?;
            // if acc_privileged_txs_len > PRIVILEGED_TX_BUDGET {
            //     warn!(
            //         "Privileged transactions budget exceeded. Any remaining blocks will be processed in the next batch."
            //     );
            //     // Break loop. Use the previous generated blobs_bundle.
            //     break;
            // }

            let state_diff = prepare_state_diff(
                block_data.header,
                &parent_db,
                &accumulator.messages,
                &accumulator.privileged_txs,
                accumulator.get_account_updates_vec(),
            )?;

            let Ok((bundle, _latest_blob_size)) = generate_blobs_bundle(&state_diff) else {
                if block_number == first_block {
                    return Err(Error::Unreachable(
                        "Not enough blob space for a single block batch. This means a block was incorrectly produced.".to_string(),
                    ));
                }
                warn!(
                    "Batch size limit reached. Any remaining blocks will be processed in the next batch."
                );
                // Break loop. Use the previous generated blobs_bundle.
                break;
            };

            // assigning the new values
            blobs_bundle = bundle;
            state_root = self.get_block_state_root(ctx, &block_data.block)?;
            current_block = block_number;
        }

        if current_block == last_committed_block {
            return Ok(None);
        }

        info!(
            privileged_tx_count = accumulator.privileged_tx_hashes.len(),
            "Added privileged transactions to batch"
        );

        Ok(Some(BatchData {
            last_block: current_block,
            state_root,
            message_hashes: accumulator.message_hashes,
            privileged_tx_hashes: accumulator.privileged_tx_hashes,
            blobs_bundle,
        }))
    }
}

#[derive(Default)]
struct BatchAccumulator {
    messages: Vec<L1Message>,
    privileged_txs: Vec<PrivilegedL2Transaction>,
    account_updates: HashMap<Address, AccountUpdate>,
    message_hashes: Vec<H256>,
    privileged_tx_hashes: Vec<H256>,
}

impl BatchAccumulator {
    fn add_block_data(
        &mut self,
        messages: Vec<L1Message>,
        privileged_txs: Vec<PrivilegedL2Transaction>,
        account_updates: Vec<AccountUpdate>,
    ) {
        self.message_hashes
            .extend(messages.iter().map(get_l1_message_hash));
        self.messages.extend(messages);

        self.privileged_tx_hashes.extend(
            privileged_txs
                .iter()
                .filter_map(|tx| tx.get_privileged_hash()),
        );
        self.privileged_txs.extend(privileged_txs);

        for update in account_updates {
            match self.account_updates.entry(update.address) {
                Entry::Occupied(mut e) => e.get_mut().merge(update),
                Entry::Vacant(v) => {
                    v.insert(update);
                }
            };
        }
    }

    fn get_account_updates_vec(&self) -> Vec<AccountUpdate> {
        self.account_updates.values().cloned().collect()
    }
}

async fn load_or_execute_updates(
    ctx: &BatchProducerContext,
    block: &Block,
    block_number: BlockNumber,
) -> Result<Vec<AccountUpdate>> {
    if let Some(account_updates) = ctx
        .rollup_store
        .get_account_updates_by_block_number(block_number)
        .await?
    {
        return Ok(account_updates);
    }

    warn!(
        "Could not find execution cache result for block {}, falling back to re-execution",
        block_number + 1
    );

    let vm_db = StoreVmDatabase::new(ctx.store.clone(), block.header.parent_hash);
    let mut vm = ctx.blockchain.new_evm(vm_db)?;
    vm.execute_block(block)?;
    vm.get_state_transitions().map_err(Error::from)
}

fn generate_blobs_bundle(state_diff: &StateDiff) -> Result<(BlobsBundle, usize)> {
    let blob_data = state_diff.encode().map_err(Error::from)?;
    let blob_size = blob_data.len();
    let blob = blobs_bundle::blob_from_bytes(blob_data).map_err(Error::from)?;
    Ok((
        BlobsBundle::create_from_blobs(&vec![blob]).map_err(Error::from)?,
        blob_size,
    ))
}

/// Prepare the state diff for the block.
fn prepare_state_diff(
    _last_header: BlockHeader,
    _db: &impl VmDatabase,
    _l1messages: &[L1Message],
    _privileged_transactions: &[PrivilegedL2Transaction],
    _account_updates: Vec<AccountUpdate>,
) -> Result<StateDiff> {
    Ok(StateDiff::default())
}

fn get_privileged_transactions() -> Vec<PrivilegedL2Transaction> {
    vec![]
}

fn get_block_l1_messages() -> Vec<L1Message> {
    vec![]
}
