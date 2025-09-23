use std::sync::Arc;

use ethrex_blockchain::Blockchain;
use ethrex_storage::Store;
use ethrex_storage_rollup::StoreRollup;

#[derive(Clone)]
pub struct BatchProducerContext {
    pub(crate) store: Store,
    pub(crate) blockchain: Arc<Blockchain>,
    pub(crate) rollup_store: StoreRollup,
}

impl BatchProducerContext {
    pub fn new(store: Store, blockchain: Arc<Blockchain>, rollup_store: StoreRollup) -> Self {
        Self {
            store,
            blockchain,
            rollup_store,
        }
    }
}
