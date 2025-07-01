use std::net::SocketAddr;

use ethrex_rpc::{RpcErr, types::transaction::SendRawTransactionRequest};
use serde_json::Value;

#[derive(Debug, PartialEq, Eq)]
pub enum SyncClientMode {
    FullNode {
        sequencer_addr: SocketAddr,
    },
    #[cfg(feature = "sequencer")]
    Sequencer {
        full_node_addrs: Vec<SocketAddr>,
    },
}

#[derive(Clone, Debug)]
pub struct SyncClient {
    #[cfg(not(feature = "sequencer"))]
    sequencer_addr: SocketAddr,
    #[cfg(feature = "sequencer")]
    full_node_addrs: Vec<SocketAddr>,
}

impl SyncClient {
    /// Create a new instance of SyncClient.
    pub fn new(mode: SyncClientMode) -> Self {
        match mode {
            #[cfg(not(feature = "sequencer"))]
            SyncClientMode::FullNode { sequencer_addr } => Self { sequencer_addr },
            #[cfg(feature = "sequencer")]
            SyncClientMode::Sequencer { full_node_addrs } => Self { full_node_addrs },
            _ => panic!("Invalid mode"),
        }
    }

    #[cfg(feature = "sequencer")]
    /// Add a full node address to the list of full nodes.
    pub fn push_full_node(&mut self, sequencer_addr: SocketAddr) {
        self.full_node_addrs.push(sequencer_addr);
    }

    /// Check if the current node is running as a sequencer.
    pub async fn is_sequencer(&self) -> bool {
        unimplemented!()
    }

    /// Forward the transaction.
    pub async fn forward_transaction(
        &self,
        _transaction: &SendRawTransactionRequest,
    ) -> Result<Value, RpcErr> {
        unimplemented!()
    }

    /// Broadcast the block.
    pub async fn broadcast_block(&self) -> Result<(), RpcErr> {
        unimplemented!()
    }
}
