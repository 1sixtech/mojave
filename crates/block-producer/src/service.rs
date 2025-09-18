use crate::{
    BlockProducerContext,
    error::{Error, Result},
    rpc::start_api,
    types::BlockProducerOptions,
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
    state_diff::{StateDiff, StateDiffError},
};
use ethrex_vm::VmDatabase;
use mojave_node_lib::{
    node::get_client_version,
    types::{MojaveNode, NodeConfigFile, NodeOptions},
    utils::{
        get_authrpc_socket_addr, get_http_socket_addr, read_jwtsecret_file, store_node_config_file,
    },
};
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    oneshot,
};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tracing::error;

pub async fn run(
    node: MojaveNode,
    node_options: &NodeOptions,
    block_producer_options: &BlockProducerOptions,
) -> Result<()> {
    let context = BlockProducerContext::new(
        node.store.clone(),
        node.blockchain.clone(),
        node.rollup_store.clone(),
        node.genesis.coinbase,
    );
    let block_time = block_producer_options.block_time;
    let mut block_producer = BlockProducer::start(context.clone(), 100);
    tokio::spawn(async move {
        loop {
            tracing::info!("Building block");
            if let Err(error) = block_producer.build_block().await {
                tracing::error!("Failed to build a block: {}", error);
            }
            tokio::time::sleep(Duration::from_millis(block_time)).await;
            tracing::info!("Building batch");
            if let Err(error) = block_producer.build_batch(&context).await {
                tracing::error!("Failed to build a batch: {}", error);
            }
        }
    });

    let local_node_record = node.local_node_record.lock().await.clone();

    let api_task = start_api(
        get_http_socket_addr(&node_options.http_addr, &node_options.http_port).await?,
        get_authrpc_socket_addr(&node_options.authrpc_addr, &node_options.authrpc_port).await?,
        node.store,
        node.blockchain,
        read_jwtsecret_file(&node_options.authrpc_jwtsecret).await?,
        node.local_p2p_node,
        local_node_record,
        node.syncer,
        node.peer_handler,
        get_client_version(),
        node.rollup_store,
        node.cancel_token.clone(),
    );
    let cancel_token = node.cancel_token.clone();
    tokio::pin!(api_task);
    tokio::select! {
        res = &mut api_task => {
            if let Err(error) = res {
                tracing::error!("API task returned error: {}", error);
            }
        }
        _ = cancel_token.cancelled() => {
            tracing::info!("Shutting down the block producer..");
            let node_config_path = PathBuf::from(node.data_dir.clone()).join("node_config.json");
            tracing::info!("Storing config at {:?}...", node_config_path);
            let node_config = NodeConfigFile::new(node.peer_table.clone(), node.local_node_record.lock().await.clone()).await;
            store_node_config_file(node_config, node_config_path).await;
            if let Err(_elapsed) = tokio::time::timeout(std::time::Duration::from_secs(10), api_task).await {
                tracing::warn!("Timed out waiting for API to stop");
            }
            tracing::info!("Successfully shut down the block producer.");
        }
    }

    Ok(())
}

#[derive(Clone)]
pub struct BlockProducer {
    sender: mpsc::Sender<Message>,
    // TODO: replace that with a real batch counter (getting the batch counter from the context/l1)
    // dummy batch counter for the moment
    batch_counter: u64,
}

impl BlockProducer {
    pub fn start(context: BlockProducerContext, channel_capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(channel_capacity);
        let mut receiver = ReceiverStream::new(receiver);

        tokio::spawn(async move {
            while let Some(message) = receiver.next().await {
                handle_message(&context, message).await;
            }

            error!("Block builder stopped because the sender dropped.");
        });
        Self {
            sender,
            batch_counter: 0,
        }
    }

    pub async fn build_block(&self) -> Result<Block> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .try_send(Message::BuildBlock(sender))
            .map_err(|error| match error {
                TrySendError::Full(_) => Error::Full,
                TrySendError::Closed(_) => Error::Stopped,
            })?;
        receiver.await?
    }

    pub async fn build_batch(&mut self, context: &BlockProducerContext) -> Result<()> {
        tracing::info!("Building batch");

        let last_batch_committed = self.batch_counter;
        let batch_to_commit = self.batch_counter + 1;
        self.batch_counter += 1;

        let last_committed_blocks = context
            .rollup_store
            .get_block_numbers_by_batch(last_batch_committed)
            .await?
            .ok_or(Error::RetrievalError(format!("Failed to get batch with batch number {batch_to_commit}. Batch is missing when it should be present. This is a bug")))?;

        let last_block = last_committed_blocks.last().ok_or(Error::RetrievalError(format!("Last committed batch ({batch_to_commit}) doesn't have any blocks. This is probably a bug."))
)?;
        let first_block_to_commit = last_block + 1;

        let (
            blobs_bundle,
            new_state_root,
            message_hashes,
            privileged_transactions_hash,
            last_block_of_batch,
        ) = self
            .prepare_batch_from_block(context, *last_block, batch_to_commit)
            .await?;

        if *last_block == last_block_of_batch {
            tracing::debug!("No new blocks to commit, skipping");
            return Ok(());
        }

        let batch = Batch {
            number: batch_to_commit,
            first_block: first_block_to_commit,
            last_block: last_block_of_batch,
            state_root: new_state_root,
            privileged_transactions_hash,
            message_hashes,
            blobs_bundle,
            commit_tx: None,
            verify_tx: None,
        };

        context.rollup_store.seal_batch(batch.clone()).await?;

        tracing::debug!(
            first_block = batch.first_block,
            last_block = batch.last_block,
            "Batch {} stored in database",
            batch.number
        );

        let fake_hash = H256::random();

        context
            .rollup_store
            .store_commit_tx_by_batch(batch_to_commit, fake_hash)
            .await?;

        Ok(())
    }

    async fn prepare_batch_from_block(
        &mut self,
        ctx: &BlockProducerContext,
        mut last_added_block_number: BlockNumber,
        batch_number: u64,
    ) -> Result<(BlobsBundle, H256, Vec<H256>, H256, BlockNumber)> {
        let first_block_of_batch = last_added_block_number + 1;
        let mut blobs_bundle = BlobsBundle::default();

        let mut acc_messages = vec![];
        let mut acc_privileged_txs = vec![];
        let mut acc_account_updates: HashMap<Address, AccountUpdate> = HashMap::new();
        let mut message_hashes = vec![];
        let mut privileged_transactions_hashes = vec![];
        let mut new_state_root = H256::default();

        tracing::info!(
            first_block_of_batch = first_block_of_batch,
            batch_number = batch_number,
            "Preparing state diff"
        );

        loop {
            let block_to_commit_number = last_added_block_number + 1;

            let Some(block_to_commit_body) =
                ctx.store.get_block_body(block_to_commit_number).await?
            else {
                tracing::debug!("No new block to commit, skipping..");
                break;
            };

            let block_to_commit_header = ctx
                .store
                .get_block_header(block_to_commit_number)
                .map_err(Error::from)?
                .ok_or(Error::FailedToGetInformationFromStorage(
                    "Failed to get_block_header() after get_block_body()".to_owned(),
                ))?;

            // Get block transactions and receipts
            let mut txs = vec![];
            let mut receipts = vec![];
            for (index, tx) in block_to_commit_body.transactions.iter().enumerate() {
                let receipt = ctx
                    .store
                    .get_receipt(block_to_commit_number, index.try_into()?)
                    .await?
                    .ok_or(Error::RetrievalError(
                        "Transactions in a block should have a receipt".to_owned(),
                    ))?;
                txs.push(tx.clone());
                receipts.push(receipt);
            }

            // TODO: replace by somthing that really do something
            // this shall collect stuff like withdrawal request stuff like that in an ETH l2 it would collect all the message from the L2 to the L1
            let messages = get_block_l1_message();
            // TODO: replace by something that really does something
            let privileged_txs = get_privileged_txs();

            // Get block account updates.
            let block_to_commit = Block::new(block_to_commit_header.clone(), block_to_commit_body);
            let account_updates = if let Some(account_updates) = ctx
                .rollup_store
                .get_account_updates_by_block_number(block_to_commit_number)
                .await?
            {
                account_updates
            } else {
                tracing::warn!(
                    "Could not find execution cache result for block {}, falling back to re-execution",
                    last_added_block_number + 1
                );

                let vm_db =
                    StoreVmDatabase::new(ctx.store.clone(), block_to_commit.header.parent_hash);
                let mut vm = ctx.blockchain.new_evm(vm_db)?;
                vm.execute_block(&block_to_commit)?;
                vm.get_state_transitions()?
            };

            acc_messages.extend(messages.clone());
            acc_privileged_txs.extend(privileged_txs.clone());
            for account in account_updates {
                let address = account.address;
                if let Some(existing) = acc_account_updates.get_mut(&address) {
                    existing.merge(account);
                } else {
                    acc_account_updates.insert(address, account);
                }
            }

            let parent_block_hash = ctx
                .store
                .get_block_header(first_block_of_batch)?
                .ok_or(Error::FailedToGetInformationFromStorage(
                    "Failed to get_block_header() of the last added block".to_owned(),
                ))?
                .parent_hash;
            let parent_db = StoreVmDatabase::new(ctx.store.clone(), parent_block_hash);

            let result = {
                // if !self.validium {
                // Prepare current state diff.
                let state_diff = prepare_state_diff(
                    block_to_commit_header,
                    &parent_db,
                    &acc_messages,
                    &acc_privileged_txs,
                    acc_account_updates.clone().into_values().collect(),
                )?;
                generate_blobs_bundle(&state_diff)
            };
            // } else {
            //     Ok((BlobsBundle::default(), 0_usize))
            // };
            //
            let Ok((bundle, latest_blob_size)) = result else {
                if block_to_commit_number == first_block_of_batch {
                    return Err(Error::Unreachable(
                        "Not enough blob space for a single block batch. This means a block was incorrectly produced.".to_string(),
                    ));
                }
                tracing::warn!(
                    "Batch size limit reached. Any remaining blocks will be processed in the next batch."
                );
                // Break loop. Use the previous generated blobs_bundle.
                break;
            };

            // Save current blobs_bundle and continue to add more blocks.
            blobs_bundle = bundle;

            privileged_transactions_hashes.extend(
                privileged_txs
                    .iter()
                    .filter_map(|tx| tx.get_privileged_hash())
                    .collect::<Vec<H256>>(),
            );

            new_state_root = ctx
                .store
                .state_trie(block_to_commit.hash())?
                .ok_or(Error::FailedToGetInformationFromStorage(
                    "Failed to get state root from storage".to_owned(),
                ))?
                .hash_no_commit();

            last_added_block_number += 1;
        }

        let privileged_transactions_hash =
            compute_privileged_transactions_hash(privileged_transactions_hashes)?;
        for msg in &acc_messages {
            message_hashes.push(get_l1_message_hash(msg));
        }
        Ok((
            blobs_bundle,
            new_state_root,
            message_hashes,
            privileged_transactions_hash,
            last_added_block_number,
        ))
    }
}

pub fn generate_blobs_bundle(state_diff: &StateDiff) -> Result<(BlobsBundle, usize)> {
    let blob_data = state_diff.encode().map_err(Error::from)?;

    let blob_size = blob_data.len();

    let blob = blobs_bundle::blob_from_bytes(blob_data).map_err(Error::from)?;

    Ok((
        BlobsBundle::create_from_blobs(&vec![blob]).map_err(Error::from)?,
        blob_size,
    ))
}

/// Prepare the state diff for the block.
pub fn prepare_state_diff(
    last_header: BlockHeader,
    db: &impl VmDatabase,
    l1messages: &[L1Message],
    privileged_transactions: &[PrivilegedL2Transaction],
    account_updates: Vec<AccountUpdate>,
) -> Result<StateDiff> {
    Ok(StateDiff::default())
    // todo!()
    // let mut modified_accounts = BTreeMap::new();
    // for account_update in account_updates {
    //     let nonce_diff = get_nonce_diff(&account_update, db)?;

    //     modified_accounts.insert(
    //         account_update.address,
    //         AccountStateDiff {
    //             new_balance: account_update.info.clone().map(|info| info.balance),
    //             nonce_diff,
    //             storage: account_update.added_storage.clone().into_iter().collect(),
    //             bytecode: account_update.code.clone(),
    //             bytecode_hash: None,
    //         },
    //     );
    // }

    // let state_diff = StateDiff {
    //     modified_accounts,
    //     version: StateDiff::default().version,
    //     last_header,
    //     l1_messages: l1messages.to_vec(),
    //     privileged_transactions: privileged_transactions
    //         .iter()
    //         .map(|tx| PrivilegedTransactionLog {
    //             address: match tx.to {
    //                 TxKind::Call(address) => address,
    //                 TxKind::Create => Address::zero(),
    //             },
    //             amount: tx.value,
    //             nonce: tx.nonce,
    //         })
    //         .collect(),
    // };

    // Ok(state_diff)
}

fn get_privileged_txs() -> Vec<PrivilegedL2Transaction> {
    vec![]
}

fn get_block_l1_message() -> Vec<L1Message> {
    vec![]
}

async fn handle_message(context: &BlockProducerContext, message: Message) {
    match message {
        Message::BuildBlock(sender) => {
            if let Err(e) = sender.send(context.build_block().await) {
                tracing::warn!(error = ?e, "Failed to send built block over channel");
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum Message {
    BuildBlock(oneshot::Sender<Result<Block>>),
}
