use ethrex_rpc::RpcApiContext as L1Context;
use ethrex_storage_rollup::StoreRollup;

#[derive(Clone, Debug)]
pub struct RpcApiContext {
    pub l1_context: L1Context,
    pub rollup_store: StoreRollup,
}
