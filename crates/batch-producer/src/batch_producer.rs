use std::sync::Arc;

use crate::{
    batch_accumulator::BatchAccumulator,
    error::{Error, Result},
    types::{BatchData, BlockData, Request},
    utils::{
        generate_blobs_bundle, get_block_l1_messages, get_privileged_transactions,
        prepare_state_diff,
    },
};

use ethrex_blockchain::{Blockchain, vm::StoreVmDatabase};
use ethrex_common::{
    H256,
    types::{
        AccountUpdate, BlobsBundle, Block, BlockNumber, PrivilegedL2Transaction, batch::Batch,
    },
};
use ethrex_l2_common::{
    l1_messages::L1Message, privileged_transactions::compute_privileged_transactions_hash,
};
use ethrex_storage::Store;
use ethrex_storage_rollup::StoreRollup;
use mojave_node_lib::types::MojaveNode;
use mojave_task::Task;
use tracing::{debug, info, warn};

pub struct BatchProducer {
    // TODO: replace that with a real batch counter (getting the batch counter from the context/l1)
    // dummy batch counter for the moment
    batch_counter: u64,

    store: Store,
    blockchain: Arc<Blockchain>,
    rollup_store: StoreRollup,
}

impl Task for BatchProducer {
    type Request = Request;
    type Response = Option<Batch>;
    type Error = Error;

    async fn on_start(&mut self) -> Result<()> {
        info!("Starting BatchProducer task");
        Ok(())
    }

    async fn handle_request(&mut self, request: Self::Request) -> Result<Self::Response> {
        match request {
            Request::BuildBatch => self.build_batch().await,
        }
    }

    async fn on_shutdown(&mut self) -> Result<()> {
        info!("Shutting down batch producer");
        Ok(())
    }
}

impl BatchProducer {
    pub fn new(node: MojaveNode, batch_counter: u64) -> Self {
        BatchProducer {
            batch_counter,
            store: node.store.clone(),
            blockchain: node.blockchain.clone(),
            rollup_store: node.rollup_store.clone(),
        }
    }

    pub async fn build_batch(&mut self) -> Result<Option<Batch>> {
        let batch_number = self.batch_counter + 1;

        debug!(
            last_commited_batch_number = self.batch_counter,
            batch_number, "Building batch"
        );

        // TODO: add a check if we already have the batch in the rollup_store ?

        let last_block = self.get_last_committed_block(self.batch_counter).await?;
        let first_block = last_block + 1;
        let batch_data = self
            .prepare_batch_from_block(last_block, first_block, batch_number)
            .await?;

        let Some(batch_data) = batch_data else {
            debug!("No new blocks to commit, skipping batch creation");
            return Ok(None);
        };

        let batch = self.create_batch(batch_number, first_block, batch_data)?;

        self.rollup_store.seal_batch(batch.clone()).await?;

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

    async fn create_parent_database(&self, first_block: BlockNumber) -> Result<StoreVmDatabase> {
        let parent_hash = self
            .store
            .get_block_header(first_block)?
            .ok_or_else(|| {
                Error::FailedToGetInformationFromStorage(format!(
                    "Failed to get block header for block {first_block}"
                ))
            })?
            .parent_hash;

        Ok(StoreVmDatabase::new(self.store.clone(), parent_hash))
    }

    fn get_block_state_root(&self, block: &Block) -> Result<H256> {
        let hash = self
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

    async fn get_block_data(&self, block_number: BlockNumber) -> Result<Option<BlockData>> {
        let Some(body) = self.store.get_block_body(block_number).await? else {
            return Ok(None);
        };

        let header = self
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
        block_data: &BlockData,
    ) -> Result<(
        Vec<L1Message>,
        Vec<PrivilegedL2Transaction>,
        Vec<AccountUpdate>,
    )> {
        let messages = get_block_l1_messages();
        let privileged_txs = get_privileged_transactions();
        let account_updates = self
            .load_or_execute_updates(&block_data.block, block_data.header.number)
            .await?;

        Ok((messages, privileged_txs, account_updates))
    }

    async fn prepare_batch_from_block(
        &mut self,
        last_committed_block: BlockNumber,
        first_block: BlockNumber,
        batch_number: u64,
    ) -> Result<Option<BatchData>> {
        info!(first_block, batch_number, "Preparing batch");

        let parent_db = self.create_parent_database(first_block).await?;
        let mut accumulator = BatchAccumulator::default();
        let mut blobs_bundle = BlobsBundle::default();
        let mut state_root = H256::default();
        let mut current_block = first_block;

        loop {
            let block_number = current_block;

            // get body and header of current block we wish to add to the batch
            let Some(block_data) = self.get_block_data(block_number).await? else {
                debug!("No more blocks available for batch");
                break;
            };

            // TODO: add gas check

            let (messages, privileged_txs, account_updates) =
                self.process_block(&block_data).await?;

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
            state_root = self.get_block_state_root(&block_data.block)?;
            current_block = block_number + 1;
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

    async fn load_or_execute_updates(
        &self,
        block: &Block,
        block_number: BlockNumber,
    ) -> Result<Vec<AccountUpdate>> {
        if let Some(account_updates) = self
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

        let vm_db = StoreVmDatabase::new(self.store.clone(), block.header.parent_hash);
        let mut vm = self.blockchain.new_evm(vm_db)?;
        vm.execute_block(block)?;
        vm.get_state_transitions().map_err(Error::from)
    }

    async fn get_last_committed_block(&self, batch_number: u64) -> Result<u64> {
        let last_committed_blocks = self
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
}
