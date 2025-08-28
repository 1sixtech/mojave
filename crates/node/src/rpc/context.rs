use ethrex_rpc::{EthClient, RpcApiContext as L1Context};
use ethrex_storage_rollup::StoreRollup;
use mojave_utils::unique_heap::AsyncUniqueHeap;

use crate::rpc::types::{OrderedBlock, PendingHeap};


#[derive(Clone, Debug)]
pub struct RpcApiContext {
    pub l1_context: L1Context,
    pub rollup_store: StoreRollup,
    pub eth_client: EthClient,
    pub block_queue: AsyncUniqueHeap<OrderedBlock, u64>,
    pub pending_signed_blocks: PendingHeap,
}
