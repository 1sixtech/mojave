#[cfg(test)]
mod tests {
    use crate::rpc::{
        context::RpcApiContext,
        tasks::spawn_block_processing_task,
        types::{OrderedBlock, PendingHeap},
    };
    use ethrex_blockchain::Blockchain;
    use ethrex_common::{
        Address, Bloom, Bytes, H256, H512, U256,
        constants::DEFAULT_OMMERS_HASH,
        types::{
            Block, BlockBody, BlockHeader, ChainConfig, ELASTICITY_MULTIPLIER, Genesis,
            INITIAL_BASE_FEE, calculate_base_fee_per_gas, compute_receipts_root,
            compute_transactions_root,
        },
    };
    use ethrex_p2p::{
        peer_handler::PeerHandler,
        sync_manager::SyncManager,
        types::{Node, NodeRecord},
    };
    use ethrex_rpc::{
        ActiveFilters, EthClient, GasTipEstimator, NodeData, RpcApiContext as L1Context,
    };
    use ethrex_storage::{EngineType, Store};
    use ethrex_storage_rollup::{EngineTypeRollup, StoreRollup};
    use mojave_utils::unique_heap::AsyncUniqueHeap;
    use std::{
        collections::{BTreeMap, HashMap},
        net::{IpAddr, Ipv4Addr},
        sync::{Arc, Mutex},
        time::Duration,
    };
    use tokio::sync::Mutex as TokioMutex;
    use tokio_util::sync::CancellationToken;

    fn build_genesis() -> Genesis {
        Genesis {
            config: ChainConfig {
                chain_id: 1,
                london_block: Some(0),
                ..Default::default()
            },
            alloc: BTreeMap::new(),
            coinbase: Address::zero(),
            difficulty: U256::zero(),
            extra_data: Bytes::new(),
            gas_limit: 30_000_000,
            nonce: 0,
            mix_hash: H256::zero(),
            timestamp: 0,
            base_fee_per_gas: None,
            blob_gas_used: None,
            excess_blob_gas: None,
            requests_hash: None,
        }
    }

    fn next_block(parent: &Block) -> Block {
        let parent_header = &parent.header;
        let base_fee = calculate_base_fee_per_gas(
            parent_header.gas_limit,
            parent_header.gas_limit,
            parent_header.gas_used,
            parent_header.base_fee_per_gas.unwrap_or(INITIAL_BASE_FEE),
            ELASTICITY_MULTIPLIER,
        )
        .unwrap();

        let header = BlockHeader {
            parent_hash: parent.hash(),
            ommers_hash: *DEFAULT_OMMERS_HASH,
            coinbase: parent_header.coinbase,
            state_root: parent_header.state_root,
            transactions_root: compute_transactions_root(&[]),
            receipts_root: compute_receipts_root(&[]),
            logs_bloom: Bloom::zero(),
            difficulty: U256::zero(),
            number: parent_header.number + 1,
            gas_limit: parent_header.gas_limit,
            gas_used: 0,
            timestamp: parent_header.timestamp + 1,
            extra_data: Bytes::new(),
            prev_randao: parent_header.prev_randao,
            nonce: 0,
            base_fee_per_gas: Some(base_fee),
            withdrawals_root: None,
            blob_gas_used: None,
            excess_blob_gas: None,
            parent_beacon_block_root: None,
            requests_hash: None,
            ..Default::default()
        };
        let body = BlockBody {
            transactions: vec![],
            ommers: vec![],
            withdrawals: None,
        };
        Block::new(header, body)
    }

    #[tokio::test]
    async fn block_processing_updates_storage_and_blockchain() {
        let store = Store::new("", EngineType::InMemory).unwrap();
        let genesis = build_genesis();
        store.add_initial_state(genesis.clone()).await.unwrap();
        let genesis_block = genesis.get_block();
        let blockchain = Arc::new(Blockchain::default_with_store(store.clone()));

        let rollup_store = StoreRollup::new("", EngineTypeRollup::InMemory).unwrap();
        rollup_store.init().await.unwrap();
        let eth_client = EthClient::new("http://localhost:8545").unwrap();

        let block_queue = AsyncUniqueHeap::new();
        let block = next_block(&genesis_block);
        block_queue.push(OrderedBlock(block.clone())).await;

        let active_filters: ActiveFilters = Arc::new(Mutex::new(HashMap::new()));
        let l1_context = L1Context {
            storage: store.clone(),
            blockchain: blockchain.clone(),
            active_filters: active_filters.clone(),
            syncer: Arc::new(SyncManager::dummy()),
            peer_handler: PeerHandler::dummy(),
            node_data: NodeData {
                jwt_secret: Bytes::new(),
                local_p2p_node: Node::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0, 0, H512::zero()),
                local_node_record: NodeRecord {
                    signature: H512::zero(),
                    seq: 0,
                    pairs: vec![],
                },
                client_version: "test".to_string(),
            },
            gas_tip_estimator: Arc::new(TokioMutex::new(GasTipEstimator::new())),
        };
        let context = RpcApiContext {
            l1_context,
            rollup_store,
            eth_client,
            block_queue: block_queue.clone(),
            pending_signed_blocks: PendingHeap::new(),
        };

        let cancel_token = CancellationToken::new();
        let handle = spawn_block_processing_task(context.clone(), cancel_token.clone());
        tokio::time::timeout(Duration::from_secs(1), async {
            while !context.block_queue.is_empty().await {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("block not processed");
        cancel_token.cancel();
        handle.await.unwrap();

        assert!(context.block_queue.is_empty().await);

        // Blockchain and storage reflect the added block
        let stored_header = context
            .l1_context
            .storage
            .get_block_header(block.header.number)
            .unwrap()
            .unwrap();
        assert_eq!(stored_header.hash(), block.hash());

        // Earliest block number updated
        let earliest = context
            .l1_context
            .storage
            .get_earliest_block_number()
            .await
            .unwrap();
        assert_eq!(earliest, block.header.number);

        // Forkchoice updated
        let canonical_hash = context
            .l1_context
            .storage
            .get_canonical_block_hash(block.header.number)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(canonical_hash, block.hash());
    }
}
