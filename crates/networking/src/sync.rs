use ethrex_rpc::{RpcErr, types::transaction::SendRawTransactionRequest};
use serde_json::Value;
use std::net::SocketAddr;

#[derive(Clone, Debug)]
pub enum SyncClient {
    FullNode { sequencer_addr: SocketAddr },
    Sequencer { full_node_addrs: Vec<SocketAddr> },
}

impl SyncClient {
    pub fn new_full_node(sequencer_addr: SocketAddr) -> Self {
        Self::FullNode { sequencer_addr }
    }

    pub fn new_sequencer(full_node_addrs: Vec<SocketAddr>) -> Self {
        Self::Sequencer { full_node_addrs }
    }

    pub fn is_sequencer(&self) -> bool {
        matches!(self, SyncClient::Sequencer { .. })
    }

    /// Add a full node address to the list of full nodes.
    pub fn push_full_node(&mut self, addr: SocketAddr) {
        if let SyncClient::Sequencer { full_node_addrs } = self {
            full_node_addrs.push(addr);
        } else {
            panic!("push_full_node called on FullNode mode");
        }
    }

    /// Forward a transaction received by a full node to a sequencer.
    pub async fn forward_transaction(
        &self,
        _transaction: &SendRawTransactionRequest,
    ) -> Result<Value, RpcErr> {
        match self {
            SyncClient::Sequencer { .. } => {
                unimplemented!("Forwarding not implemented for Sequencer mode");
            }
            SyncClient::FullNode { sequencer_addr } => {
                println!("Forwarding transaction to {sequencer_addr}");
                Ok(serde_json::json!({"status": "forwarded"}))
            }
        }
    }

    /// Forward a block from a sequencer to a full node once he has build it.
    pub async fn broadcast_block(&self) -> Result<(), RpcErr> {
        match self {
            SyncClient::Sequencer { .. } => {
                println!("Broadcasting block to full nodes...");
                Ok(())
            }
            SyncClient::FullNode { .. } => {
                println!("Full node can't broadcast blocks.");
                Ok(())
            }
        }
    }
}
