use std::sync::Arc;

use ethrex_blockchain::Blockchain;
use ethrex_p2p::{
    kademlia::Kademlia,
    network::P2PContext,
    rlpx::l2::l2_connection::P2PBasedContext,
    types::{Node, NodeRecord},
};
use ethrex_storage::Store;
use mojave_utils::network::Network;
use secp256k1::SecretKey;
use tokio::sync::Mutex;
use tokio_util::task::TaskTracker;

use crate::{error::Result, node::get_client_version, utils::get_bootnodes};

#[expect(clippy::too_many_arguments)]
pub async fn start_network(
    bootnodes: Vec<Node>,
    network: &Network,
    data_dir: &str,
    local_p2p_node: Node,
    local_node_record: Arc<Mutex<NodeRecord>>,
    signer: SecretKey,
    peer_table: Kademlia,
    store: Store,
    tracker: TaskTracker,
    blockchain: Arc<Blockchain>,
    based_context: Option<P2PBasedContext>,
) -> Result<P2PContext> {
    let bootnodes = get_bootnodes(bootnodes, network, data_dir).await;

    let context = P2PContext::new(
        local_p2p_node,
        local_node_record,
        tracker.clone(),
        signer,
        peer_table.clone(),
        store,
        blockchain.clone(),
        get_client_version(),
        based_context,
    )
    .await?;
    // TODO: remove this and get a real sync going on and let etherx set blockchain synced
    blockchain.set_synced();
    ethrex_p2p::start_network(context.clone(), bootnodes.clone()).await?;

    tracker.spawn(ethrex_p2p::periodically_show_peer_stats(
        blockchain,
        peer_table.peers,
    ));
    Ok(context)
}
