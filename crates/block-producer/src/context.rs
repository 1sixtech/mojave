use crate::block_producer::BlockProducerError;
use ethrex_blockchain::{
    Blockchain,
    constants::TX_GAS_COST,
    error::ChainError,
    fork_choice::apply_fork_choice,
    payload::{
        BuildPayloadArgs, HeadTransaction, PayloadBuildContext, PayloadBuildResult,
        TransactionQueue, apply_plain_transaction, calc_gas_limit,
    },
    validate_block,
};
use ethrex_common::{
    Address, Bloom, Bytes, H256, U256,
    constants::{DEFAULT_OMMERS_HASH, DEFAULT_REQUESTS_HASH},
    types::{
        Block, BlockBody, BlockHeader, Receipt, SAFE_BYTES_PER_BLOB, Transaction,
        calc_excess_blob_gas, calculate_base_fee_per_gas, compute_receipts_root,
        compute_transactions_root, compute_withdrawals_root,
    },
};
use ethrex_l2_common::{
    l1_messages::get_block_l1_messages,
    state_diff::{
        AccountStateDiff, BLOCK_HEADER_LEN, L1MESSAGE_LOG_LEN, PRIVILEGED_TX_LOG_LEN,
        SIMPLE_TX_STATE_DIFF_SIZE, StateDiffError,
    },
};
use ethrex_storage::Store;
use ethrex_storage_rollup::StoreRollup;
use ethrex_vm::{BlockExecutionResult, Evm, EvmError};
use std::{
    collections::{BTreeMap, HashMap},
    ops::Div,
    sync::Arc,
    time::{Instant, SystemTime},
};
use tracing::{debug, error, info};

#[derive(Clone)]
pub struct BlockProducerContext {
    store: Store,
    blockchain: Arc<Blockchain>,
    rollup_store: StoreRollup,
    coinbase_address: Address,
    elasticity_multiplier: u64,
}

impl BlockProducerContext {
    pub fn new(
        store: Store,
        blockchain: Arc<Blockchain>,
        rollup_store: StoreRollup,
        coinbase_address: Address,
        elasticity_multiplier: u64,
    ) -> Self {
        Self {
            store,
            blockchain,
            rollup_store,
            coinbase_address,
            elasticity_multiplier,
        }
    }

    pub(crate) async fn build_block(&self) -> Result<Block, BlockProducerError> {
        let version = 3;
        let head_header = {
            let current_block_number = self.store.get_latest_block_number().await?;
            self.store
                .get_block_header(current_block_number)?
                .ok_or(BlockProducerError::StorageDataIsNone)?
        };
        let head_hash = head_header.hash();
        let head_beacon_block_root = H256::zero();

        // The proposer leverages the execution payload framework used for the engine API,
        // but avoids calling the API methods and unnecesary re-execution.

        info!("Producing block");
        debug!("Head block hash: {head_hash:#x}");

        // Proposer creates a new payload
        let args = BuildPayloadArgs {
            parent: head_hash,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs(),
            fee_recipient: self.coinbase_address,
            random: H256::zero(),
            withdrawals: Default::default(),
            beacon_root: Some(head_beacon_block_root),
            version,
            elasticity_multiplier: self.elasticity_multiplier,
        };
        let payload = self.create_payload(&args)?;

        // Blockchain builds the payload from mempool txs and executes them
        let payload_build_result = self.build_payload(payload).await?;
        info!(
            "Built payload for new block {}",
            payload_build_result.payload.header.number
        );

        // Blockchain stores block
        let block = payload_build_result.payload;
        let chain_config = self.store.get_chain_config()?;
        validate_block(
            &block,
            &head_header,
            &chain_config,
            self.elasticity_multiplier,
        )?;

        let account_updates = payload_build_result.account_updates;

        let execution_result = BlockExecutionResult {
            receipts: payload_build_result.receipts,
            requests: Vec::new(),
        };

        let account_updates_list = self
            .store
            .apply_account_updates_batch(block.header.parent_hash, &account_updates)
            .await?
            .ok_or(BlockProducerError::ChainError(ChainError::ParentNotFound))?;

        self.blockchain
            .store_block(&block, account_updates_list, execution_result)
            .await?;
        info!("Stored new block {:x}", block.hash());
        // WARN: We're not storing the payload into the Store because there's no use to it by the L2 for now.

        self.rollup_store
            .store_account_updates_by_block_number(block.header.number, account_updates)
            .await?;

        // Make the new head be part of the canonical chain
        apply_fork_choice(&self.store, block.hash(), block.hash(), block.hash()).await?;

        // metrics!(
        //     let _ = METRICS_BLOCKS
        //     .set_block_number(block.header.number)
        //     .inspect_err(|e| {
        //         tracing::error!("Failed to set metric: block_number {}", e.to_string())
        //     });
        //     #[allow(clippy::as_conversions)]
        //     let tps = block.body.transactions.len() as f64 / (state.block_time_ms as f64 / 1000_f64);
        //     METRICS_TX.set_transactions_per_second(tps);
        // );
        Ok(block)
    }

    /// Creates a new payload based on the payload arguments
    /// Basic payload block building, can and should be improved
    fn create_payload(&self, args: &BuildPayloadArgs) -> Result<Block, BlockProducerError> {
        let parent_block = self
            .store
            .get_block_header_by_hash(args.parent)?
            .ok_or_else(|| ChainError::ParentNotFound)?;
        let chain_config = self.store.get_chain_config()?;
        let gas_limit = calc_gas_limit(parent_block.gas_limit);
        let excess_blob_gas = chain_config
            .get_fork_blob_schedule(args.timestamp)
            .map(|schedule| {
                calc_excess_blob_gas(
                    parent_block.excess_blob_gas.unwrap_or_default(),
                    parent_block.blob_gas_used.unwrap_or_default(),
                    schedule.target,
                )
            });

        let header = BlockHeader {
            parent_hash: args.parent,
            ommers_hash: *DEFAULT_OMMERS_HASH,
            coinbase: args.fee_recipient,
            state_root: parent_block.state_root,
            transactions_root: compute_transactions_root(&[]),
            receipts_root: compute_receipts_root(&[]),
            logs_bloom: Bloom::default(),
            difficulty: U256::zero(),
            number: parent_block.number.saturating_add(1),
            gas_limit,
            gas_used: 0,
            timestamp: args.timestamp,
            // TODO: should use builder config's extra_data
            extra_data: Bytes::new(),
            prev_randao: args.random,
            nonce: 0,
            base_fee_per_gas: calculate_base_fee_per_gas(
                gas_limit,
                parent_block.gas_limit,
                parent_block.gas_used,
                parent_block.base_fee_per_gas.unwrap_or_default(),
                args.elasticity_multiplier,
            ),
            withdrawals_root: chain_config
                .is_shanghai_activated(args.timestamp)
                .then_some(compute_withdrawals_root(
                    args.withdrawals.as_ref().unwrap_or(&Vec::new()),
                )),
            blob_gas_used: chain_config
                .is_cancun_activated(args.timestamp)
                .then_some(0),
            excess_blob_gas,
            parent_beacon_block_root: args.beacon_root,
            requests_hash: chain_config
                .is_prague_activated(args.timestamp)
                .then_some(*DEFAULT_REQUESTS_HASH),
            ..Default::default()
        };

        let body = BlockBody {
            transactions: Vec::new(),
            ommers: Vec::new(),
            withdrawals: args.withdrawals.clone(),
        };

        // Delay applying withdrawals until the payload is requested and built
        Ok(Block::new(header, body))
    }

    /// L2 payload builder
    /// Completes the payload building process, return the block value
    /// Same as `blockchain::build_payload` without applying system operations and using a different `fill_transactions`
    async fn build_payload(
        &self,
        payload: Block,
    ) -> Result<PayloadBuildResult, BlockProducerError> {
        let since = Instant::now();
        let gas_limit = payload.header.gas_limit;

        debug!("Building payload");
        let mut context = PayloadBuildContext::new(
            payload,
            self.blockchain.evm_engine,
            &self.store,
            self.blockchain.r#type.clone(),
        )?;

        self.fill_transactions(&mut context).await?;
        self.blockchain.finalize_payload(&mut context).await?;

        let interval = Instant::now().duration_since(since).as_millis();
        info!("[METRIC] BUILDING PAYLOAD TOOK: {interval} ms");
        #[allow(clippy::as_conversions)]
        if let Some(gas_used) = gas_limit.checked_sub(context.remaining_gas) {
            let as_gigas = (gas_used as f64).div(10_f64.powf(9_f64));

            if interval != 0 {
                let throughput = (as_gigas) / (interval as f64) * 1000_f64;
                tracing::info!(
                    "[METRIC] BLOCK BUILDING THROUGHPUT: {throughput} Gigagas/s TIME SPENT: {interval} msecs"
                );
                // metrics!(METRICS_BLOCKS.set_latest_gigagas(throughput));
            } else {
                // metrics!(METRICS_BLOCKS.set_latest_gigagas(0_f64));
            }
        }

        // metrics!(
        //     #[allow(clippy::as_conversions)]
        //     METRICS_BLOCKS.set_latest_block_gas_limit(
        //         ((gas_limit - context.remaining_gas) as f64 / gas_limit as f64) * 100_f64
        //     );
        //     // L2 does not allow for blob transactions so the blob pool can be ignored
        //     let (tx_pool_size, _blob_pool_size) = blockchain
        //         .mempool
        //         .get_mempool_size()
        //         .inspect_err(|e| tracing::error!("Failed to get metrics for: mempool size {}", e.to_string()))
        //         .unwrap_or((0_usize, 0_usize));
        //     let _ = METRICS_TX
        //         .set_mempool_tx_count(tx_pool_size, false)
        //         .inspect_err(|e| tracing::error!("Failed to set metrics for: blob tx mempool size {}", e.to_string()));
        // );

        Ok(context.into())
    }

    /// Same as `blockchain::fill_transactions` but enforces that the `StateDiff` size
    /// stays within the blob size limit after processing each transaction.
    async fn fill_transactions(
        &self,
        context: &mut PayloadBuildContext,
    ) -> Result<(), BlockProducerError> {
        // version (u8) + header fields (struct) + messages_len (u16) + deposits_len (u16) + accounts_diffs_len (u16)
        let mut acc_size_without_accounts = 1 + *BLOCK_HEADER_LEN + 2 + 2 + 2;
        let mut size_accounts_diffs = 0;
        let mut account_diffs = HashMap::new();

        let chain_config = self.store.get_chain_config()?;

        debug!("Fetching transactions from mempool");
        // Fetch mempool transactions
        let latest_block_number = self.store.get_latest_block_number().await?;
        let mut txs = self.fetch_mempool_transactions(context)?;
        // Execute and add transactions to payload (if suitable)
        loop {
            // Check if we have enough gas to run more transactions
            if context.remaining_gas < TX_GAS_COST {
                debug!("No more gas to run transactions");
                break;
            };

            // Check if we have enough space for the StateDiff to run more transactions
            if acc_size_without_accounts + size_accounts_diffs + SIMPLE_TX_STATE_DIFF_SIZE
                > SAFE_BYTES_PER_BLOB
            {
                debug!("No more StateDiff space to run transactions");
                break;
            };

            // Fetch the next transaction
            let Some(head_tx) = txs.peek() else {
                break;
            };

            // Check if we have enough gas to run the transaction
            if context.remaining_gas < head_tx.tx.gas_limit() {
                debug!(
                    "Skipping transaction: {}, no gas left",
                    head_tx.tx.compute_hash()
                );
                // We don't have enough gas left for the transaction, so we skip all txs from this account
                txs.pop();
                continue;
            }

            // TODO: maybe fetch hash too when filtering mempool so we don't have to compute it here (we can do this in the same refactor as adding timestamp)
            let tx_hash = head_tx.tx.compute_hash();

            // Check whether the tx is replay-protected
            if head_tx.tx.protected() && !chain_config.is_eip155_activated(context.block_number()) {
                // Ignore replay protected tx & all txs from the sender
                // Pull transaction from the mempool
                debug!("Ignoring replay-protected transaction: {}", tx_hash);
                txs.pop();
                self.blockchain.remove_transaction_from_pool(&tx_hash)?;
                continue;
            }

            let maybe_sender_acc_info = self
                .store
                .get_account_info(latest_block_number, head_tx.tx.sender())
                .await?;

            if let Some(acc_info) = maybe_sender_acc_info
                && head_tx.nonce() < acc_info.nonce
                && !head_tx.is_privileged()
            {
                debug!("Removing transaction with nonce too low from mempool: {tx_hash:#x}");
                txs.pop();
                self.blockchain.remove_transaction_from_pool(&tx_hash)?;
                continue;
            }

            // Execute tx
            let receipt = match apply_plain_transaction(&head_tx, context) {
                Ok(receipt) => receipt,
                Err(e) => {
                    debug!("Failed to execute transaction: {}, {e}", tx_hash);
                    // metrics!(METRICS_TX.inc_tx_errors(e.to_metric()));

                    // Ignore following txs from sender
                    txs.pop();
                    continue;
                }
            };

            let account_diffs_in_tx = self.get_account_diffs_in_tx(context)?;
            let merged_diffs = self.merge_diffs(&account_diffs, account_diffs_in_tx);

            let (tx_size_without_accounts, new_accounts_diff_size) = self.calculate_tx_diff_size(
                &merged_diffs,
                &head_tx,
                &receipt,
                *PRIVILEGED_TX_LOG_LEN,
                *L1MESSAGE_LOG_LEN,
            )?;

            if acc_size_without_accounts + tx_size_without_accounts + new_accounts_diff_size
                > SAFE_BYTES_PER_BLOB
            {
                debug!(
                    "No more StateDiff space to run this transactions. Skipping transaction: {:?}",
                    tx_hash
                );
                txs.pop();

                // This transaction state change is too big, we need to undo it.
                context.vm.undo_last_tx()?;

                continue;
            }

            txs.shift()?;
            // Pull transaction from the mempool
            self.blockchain
                .remove_transaction_from_pool(&head_tx.tx.compute_hash())?;

            // We only add the messages and deposits length because the accounts diffs may change
            acc_size_without_accounts += tx_size_without_accounts;
            size_accounts_diffs = new_accounts_diff_size;
            // Include the new accounts diffs
            account_diffs = merged_diffs;
            // Add transaction to block
            debug!("Adding transaction: {} to payload", tx_hash);
            context.payload.body.transactions.push(head_tx.into());
            // Save receipt for hash calculation
            context.receipts.push(receipt);
        }

        // metrics!(
        //     context
        //         .payload
        //         .body
        //         .transactions
        //         .iter()
        //         .for_each(|tx| METRICS_TX.inc_tx_with_type(MetricsTxType(tx.tx_type())))
        // );

        Ok(())
    }

    // TODO: Once #2857 is implemented, we can completely ignore the blobs pool.
    fn fetch_mempool_transactions(
        &self,
        context: &mut PayloadBuildContext,
    ) -> Result<TransactionQueue, BlockProducerError> {
        let (plain_txs, mut blob_txs) = self.blockchain.fetch_mempool_transactions(context)?;
        while let Some(blob_tx) = blob_txs.peek() {
            let tx_hash = blob_tx.compute_hash();
            self.blockchain.remove_transaction_from_pool(&tx_hash)?;
            blob_txs.pop();
        }
        Ok(plain_txs)
    }

    /// Returns the state diffs introduced by the transaction by comparing the call frame backup
    /// (which holds the state before executing the transaction) with the current state of the cache
    /// (which contains all the writes performed by the transaction).
    fn get_account_diffs_in_tx(
        &self,
        context: &PayloadBuildContext,
    ) -> Result<HashMap<Address, AccountStateDiff>, BlockProducerError> {
        let mut modified_accounts = HashMap::new();
        match &context.vm {
            Evm::REVM { .. } => {
                return Err(BlockProducerError::EvmError(EvmError::InvalidEVM(
                    "REVM not supported for L2".to_string(),
                )));
            }
            Evm::LEVM { db, .. } => {
                let transaction_backup = db.get_tx_backup().map_err(|e| {
                    BlockProducerError::FailedToGetDataFrom(format!("TransactionBackup: {e}"))
                })?;
                // First we add the account info
                for (address, original_account) in transaction_backup.original_accounts_info.iter()
                {
                    let new_account = db.current_accounts_state.get(address).ok_or(
                        BlockProducerError::FailedToGetDataFrom("DB Cache".to_owned()),
                    )?;

                    let nonce_diff: u16 = (new_account.info.nonce - original_account.info.nonce)
                        .try_into()
                        .map_err(BlockProducerError::TryIntoError)?;

                    let new_balance = if new_account.info.balance != original_account.info.balance {
                        Some(new_account.info.balance)
                    } else {
                        None
                    };

                    let account_state_diff = AccountStateDiff {
                        new_balance,
                        nonce_diff,
                        storage: BTreeMap::new(), // We add the storage later
                        bytecode: None,
                        bytecode_hash: Some(new_account.info.code_hash),
                    };

                    modified_accounts.insert(*address, account_state_diff);
                }

                // Then if there is any storage change, we add it to the account state diff
                for (address, original_storage_slots) in
                    transaction_backup.original_account_storage_slots.iter()
                {
                    let account_info = db.current_accounts_state.get(address).ok_or(
                        BlockProducerError::FailedToGetDataFrom("DB Cache".to_owned()),
                    )?;

                    let mut added_storage = BTreeMap::new();
                    for key in original_storage_slots.keys() {
                        added_storage.insert(
                            *key,
                            *account_info.storage.get(key).ok_or(
                                BlockProducerError::FailedToGetDataFrom(
                                    "Account info Storage".to_owned(),
                                ),
                            )?,
                        );
                    }
                    if let Some(account_state_diff) = modified_accounts.get_mut(address) {
                        account_state_diff.storage = added_storage;
                    } else {
                        // If the account is not in the modified accounts, we create a new one
                        let account_state_diff = AccountStateDiff {
                            new_balance: None,
                            nonce_diff: 0,
                            storage: added_storage,
                            bytecode: None,
                            bytecode_hash: None,
                        };
                        modified_accounts.insert(*address, account_state_diff);
                    }
                }
            }
        }
        Ok(modified_accounts)
    }

    /// Combines the diffs from the current transaction with the existing block diffs.
    /// Transaction diffs represent state changes from the latest transaction execution,
    /// while previous diffs accumulate all changes included in the block so far.
    fn merge_diffs(
        &self,
        previous_diffs: &HashMap<Address, AccountStateDiff>,
        tx_diffs: HashMap<Address, AccountStateDiff>,
    ) -> HashMap<Address, AccountStateDiff> {
        let mut merged_diffs = previous_diffs.clone();
        for (address, diff) in tx_diffs {
            if let Some(existing_diff) = merged_diffs.get_mut(&address) {
                // New balance could be None if a transaction didn't change the balance
                // but we want to keep the previous changes made in a transaction included in the block
                existing_diff.new_balance = diff.new_balance.or(existing_diff.new_balance);

                // We add the nonce diff to the existing one to keep track of the total nonce diff
                existing_diff.nonce_diff += diff.nonce_diff;

                // we need to overwrite only the new storage storage slot with the new values
                existing_diff.storage.extend(diff.storage);

                // Take the bytecode from the tx diff if present, avoiding clone if not needed
                if diff.bytecode.is_some() {
                    existing_diff.bytecode = diff.bytecode;
                }

                // Take the new bytecode hash if it is present
                existing_diff.bytecode_hash = diff.bytecode_hash.or(existing_diff.bytecode_hash);
            } else {
                merged_diffs.insert(address, diff);
            }
        }
        merged_diffs
    }

    /// Calculates the size of the state diffs introduced by the transaction, including
    /// the size of messages and deposits logs for this transaction, and the total
    /// size of all account diffs accumulated so far in the block.
    /// This is necessary because each transaction can modify accounts that were already
    /// changed by previous transactions, so we must recalculate the total diff size each time.
    fn calculate_tx_diff_size(
        &self,
        merged_diffs: &HashMap<Address, AccountStateDiff>,
        head_tx: &HeadTransaction,
        receipt: &Receipt,
        deposits_log_len: usize,
        messages_log_len: usize,
    ) -> Result<(usize, usize), BlockProducerError> {
        let mut tx_state_diff_size = 0;
        let mut new_accounts_diff_size = 0;

        for (address, diff) in merged_diffs.iter() {
            let encoded = match diff.encode(address) {
                Ok(encoded) => encoded,
                Err(StateDiffError::EmptyAccountDiff) => {
                    debug!("Skipping empty account diff for address: {address}");
                    continue;
                }
                Err(e) => {
                    error!("Failed to encode account state diff: {e}");
                    return Err(BlockProducerError::FailedToEncodeAccountStateDiff(e));
                }
            };
            new_accounts_diff_size += encoded.len();
        }

        if self.is_deposit_l2(head_tx) {
            tx_state_diff_size += deposits_log_len;
        }
        tx_state_diff_size +=
            get_block_l1_messages(std::slice::from_ref(receipt)).len() * messages_log_len;

        Ok((tx_state_diff_size, new_accounts_diff_size))
    }

    fn is_deposit_l2(&self, tx: &Transaction) -> bool {
        matches!(tx, Transaction::PrivilegedL2Transaction(_tx))
    }
}
