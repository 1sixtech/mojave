use ethrex_rpc::{RpcErr, types::transaction::SendRawTransactionRequest};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct SyncClient {}

impl SyncClient {
    /// Check if the current node is running as a sequencer.
    pub async fn is_sequencer(&self) -> bool {
        unimplemented!()
    }

    /// Forward the transaction.
    pub async fn forward_transaction(
        &self,
        transaction: SendRawTransactionRequest,
    ) -> Result<Value, RpcErr> {
        unimplemented!()
    }

    /// Broadcast the block.
    pub async fn broadcast_block(&self) -> Result<(), RpcErr> {
        unimplemented!()
    }
}
