pub mod block;
pub mod transaction;
pub mod types;

use crate::rpc::{
    block::SendBroadcastBlockRequest, transaction::SendRawTransactionRequest, types::OrderedBlock,
};
use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use ethrex_blockchain::Blockchain;
use ethrex_common::Bytes;
use ethrex_p2p::{
    peer_handler::PeerHandler,
    sync_manager::SyncManager,
    types::{Node, NodeRecord},
};
use ethrex_rpc::{
    ActiveFilters, EthClient, GasTipEstimator, NodeData, RpcApiContext as L1Context, RpcErr,
    RpcRequestWrapper, rpc_response,
    utils::{RpcRequest, RpcRequestId},
};
use ethrex_storage::Store;
use ethrex_storage_rollup::StoreRollup;
use mojave_chain_utils::unique_heap::AsyncUniqueHeap;
use serde_json::Value;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, sync::Mutex as TokioMutex, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use tracing::info;

pub const FILTER_DURATION: Duration = {
    if cfg!(test) {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(5 * 60)
    }
};

#[derive(Clone, Debug)]
pub struct RpcApiContext {
    pub l1_context: L1Context,
    pub rollup_store: StoreRollup,
    pub eth_client: EthClient,
    pub block_queue: AsyncUniqueHeap<OrderedBlock, u64>,
}

#[expect(clippy::too_many_arguments)]
pub async fn start_api(
    http_addr: SocketAddr,
    authrpc_addr: SocketAddr,
    storage: Store,
    blockchain: Arc<Blockchain>,
    jwt_secret: Bytes,
    local_p2p_node: Node,
    local_node_record: NodeRecord,
    syncer: SyncManager,
    peer_handler: PeerHandler,
    client_version: String,
    rollup_store: StoreRollup,
    eth_client: EthClient,
    block_queue: AsyncUniqueHeap<OrderedBlock, u64>,
    shutdown_token: CancellationToken,
) -> Result<(), RpcErr> {
    let active_filters = Arc::new(Mutex::new(HashMap::new()));
    let context = RpcApiContext {
        l1_context: L1Context {
            storage,
            blockchain,
            active_filters: active_filters.clone(),
            syncer: Arc::new(syncer),
            peer_handler,
            node_data: NodeData {
                jwt_secret,
                local_p2p_node,
                local_node_record,
                client_version,
            },
            gas_tip_estimator: Arc::new(TokioMutex::new(GasTipEstimator::new())),
        },
        rollup_store,
        eth_client,
        block_queue,
    };

    // Periodically clean up the active filters for the filters endpoints.
    let filter_handle = spawn_filter_cleanup_task(active_filters.clone(), shutdown_token.clone());
    let block_handle = spawn_block_processing_task(context.clone(), shutdown_token.clone());

    // All request headers allowed.
    // All methods allowed.
    // All origins allowed.
    // All headers exposed.
    let cors = CorsLayer::permissive();

    let http_router = Router::new()
        .route("/", post(handle_http_request))
        .layer(cors)
        .with_state(context.clone());
    let http_listener = TcpListener::bind(http_addr)
        .await
        .map_err(|error| RpcErr::Internal(error.to_string()))?;
    let http_server = axum::serve(http_listener, http_router)
        .with_graceful_shutdown(ethrex_rpc::shutdown_signal())
        .into_future();
    info!("Starting HTTP server at {http_addr}");

    info!("Not starting Auth-RPC server. The address passed as argument is {authrpc_addr}");

    let _ = tokio::try_join!(
        async {
            http_server
                .await
                .map_err(|e| RpcErr::Internal(e.to_string()))
        },
        async {
            filter_handle
                .await
                .map_err(|e| RpcErr::Internal(e.to_string()))
        },
        async {
            block_handle
                .await
                .map_err(|e| RpcErr::Internal(e.to_string()))
        },
    )
    .inspect_err(|e| info!("Error shutting down servers: {e:?}"));

    Ok(())
}

fn spawn_filter_cleanup_task(
    active_filters: ActiveFilters,
    shutdown_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let mut interval = tokio::time::interval(FILTER_DURATION);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    tracing::info!("Running filter clean task");
                    ethrex_rpc::clean_outdated_filters(active_filters.clone(), FILTER_DURATION);
                    tracing::info!("Filter clean task complete");
                }
                _ = shutdown_token.cancelled() => {
                    tracing::info!("Shutting down filter clean task");
                    break;
                }
            }
        }
    })
}

fn spawn_block_processing_task(
    context: RpcApiContext,
    shutdown_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        tracing::info!("Starting block processing loop");
        loop {
            tokio::select! {
                block = context.block_queue.pop_wait() => {
                    let added_block = context.l1_context.blockchain.add_block(&block.0).await;
                    if let Err(added_block) = added_block {
                        tracing::error!(error= %added_block, "failed to add block to blockchain");
                        continue;
                    }

                    let update_block_number = context
                        .l1_context
                        .storage
                        .update_earliest_block_number(block.0.header.number)
                        .await;
                    if let Err(update_block_number) = update_block_number {
                        tracing::error!(error = %update_block_number, "failed to update earliest block number");
                    }

                    let forkchoice_context = context
                        .l1_context
                        .storage
                        .forkchoice_update(
                            None,
                            block.0.header.number,
                            block.0.header.hash(),
                            None,
                            None,
                        )
                        .await;
                    if let Err(forkchoice_context) = forkchoice_context {
                        tracing::error!(error = %forkchoice_context, "failed to update forkchoice");
                    }
                }
                _ = shutdown_token.cancelled() => {
                    tracing::info!("Shutting down block processing loop");
                    break;
                }
            }
        }
    })
}

async fn handle_http_request(
    State(service_context): State<RpcApiContext>,
    body: String,
) -> Result<Json<Value>, StatusCode> {
    let res = match serde_json::from_str::<RpcRequestWrapper>(&body) {
        Ok(RpcRequestWrapper::Single(request)) => {
            let res = map_http_requests(&request, service_context).await;
            rpc_response(request.id, res).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        Ok(RpcRequestWrapper::Multiple(requests)) => {
            let mut responses = Vec::new();
            for req in requests {
                let res = map_http_requests(&req, service_context.clone()).await;
                responses.push(rpc_response(req.id, res).map_err(|_| StatusCode::BAD_REQUEST)?);
            }
            serde_json::to_value(responses).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        Err(_) => rpc_response(
            RpcRequestId::String("".to_string()),
            Err(RpcErr::BadParams("Invalid request body".to_string())),
        )
        .map_err(|_| StatusCode::BAD_REQUEST)?,
    };
    Ok(Json(res))
}

async fn map_http_requests(req: &RpcRequest, context: RpcApiContext) -> Result<Value, RpcErr> {
    match RpcNamespace::resolve_namespace(req) {
        Ok(RpcNamespace::Eth) => map_eth_requests(req, context).await,
        Ok(RpcNamespace::Mojave) => map_mojave_requests(req, context).await,
        Err(error) => Err(error),
    }
}

pub async fn map_eth_requests(req: &RpcRequest, context: RpcApiContext) -> Result<Value, RpcErr> {
    match req.method.as_str() {
        "eth_sendRawTransaction" => SendRawTransactionRequest::call(req, context).await,
        _others => ethrex_rpc::map_eth_requests(req, context.l1_context).await,
    }
}

pub async fn map_mojave_requests(
    req: &RpcRequest,
    context: RpcApiContext,
) -> Result<Value, RpcErr> {
    match req.method.as_str() {
        "mojave_sendBroadcastBlock" => SendBroadcastBlockRequest::call(req, context).await,
        others => Err(RpcErr::MethodNotFound(others.to_owned())),
    }
}

pub enum RpcNamespace {
    Eth,
    Mojave,
}

impl RpcNamespace {
    pub fn resolve_namespace(request: &RpcRequest) -> Result<Self, RpcErr> {
        let mut parts = request.method.split('_');
        let Some(namespace) = parts.next() else {
            return Err(RpcErr::MethodNotFound(request.method.clone()));
        };
        match namespace {
            "eth" => Ok(Self::Eth),
            "mojave" => Ok(Self::Mojave),
            _others => Err(RpcErr::MethodNotFound(request.method.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    use ethrex_rpc::{ActiveFilters, GasTipEstimator, NodeData};
    use ethrex_storage::{EngineType, Store};
    use ethrex_storage_rollup::{EngineTypeRollup, StoreRollup};
    use mojave_chain_utils::unique_heap::AsyncUniqueHeap;
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
