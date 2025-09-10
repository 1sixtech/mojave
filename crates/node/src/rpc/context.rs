use crate::pending_heap::PendingHeap;
use ethrex_rpc::{RpcApiContext as L1Context};
use ethrex_storage_rollup::StoreRollup;
use mojave_utils::{ordered_block::OrderedBlock, unique_heap::AsyncUniqueHeap};

#[derive(Clone, Debug)]
pub struct RpcApiContext {
    pub l1_context: L1Context,
    pub rollup_store: StoreRollup,
    pub block_queue: AsyncUniqueHeap<OrderedBlock, u64>,
    pub pending_signed_blocks: PendingHeap,
}
