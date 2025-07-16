use std::{
    fs, io,
    net::{Ipv4Addr, SocketAddr, ToSocketAddrs},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use ethrex::utils::{get_client_version, read_jwtsecret_file, read_node_config_file};
use ethrex_blockchain::Blockchain;
use ethrex_common::Address;
use ethrex_p2p::{
    kademlia::KademliaTable,
    network::{public_key_from_signing_key, P2PContext},
    peer_handler::PeerHandler,
    sync_manager::SyncManager,
    types::{Node, NodeRecord},
};
use ethrex_rpc::EthClient;
use ethrex_storage::Store;
use ethrex_storage_rollup::{EngineTypeRollup, StoreRollup};
use k256::ecdsa::SigningKey;
use local_ip_address::local_ip;
use mojave_networking::rpc::clients::mojave::Client;
use tokio::sync::Mutex;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    full_node_options::FullNodeOptions,
    networks::{self, Network},
    options::Options,
    sequencer_options::SequencerOpts,
};

pub fn get_bootnodes(opts: &Options, network: &Network, data_dir: &str) -> Vec<Node> {
    let mut bootnodes: Vec<Node> = opts.bootnodes.clone();

    match network {
        Network::Mainnet => {
            tracing::info!("Adding mainnet preset bootnodes");
            bootnodes.extend(networks::MAINNET_BOOTNODES.clone());
        }
        Network::Testnet => {
            tracing::info!("Adding testnet preset bootnodes");
            bootnodes.extend(networks::TESTNET_BOOTNODES.clone());
        }
        _ => {}
    }

    if bootnodes.is_empty() {
        tracing::warn!(
            "No bootnodes specified. This node will not be able to connect to the network."
        );
    }

    let config_file = PathBuf::from(data_dir.to_owned() + "/node_config.json");

    tracing::info!("Reading known peers from config file {:?}", config_file);

    match read_node_config_file(config_file) {
        Ok(ref mut config) => bootnodes.append(&mut config.known_peers),
        Err(e) => tracing::error!("Could not read from peers file: {e}"),
    };

    bootnodes
}

#[allow(clippy::too_many_arguments)]
pub async fn init_network(
    opts: &Options,
    network: &Network,
    data_dir: &str,
    local_p2p_node: Node,
    local_node_record: Arc<Mutex<NodeRecord>>,
    signer: SigningKey,
    peer_table: Arc<Mutex<KademliaTable>>,
    store: Store,
    tracker: TaskTracker,
    blockchain: Arc<Blockchain>,
) {
    if opts.dev {
        tracing::error!("Binary wasn't built with The feature flag `dev` enabled.");
        panic!(
            "Build the binary with the `dev` feature in order to use the `--dev` cli's argument."
        );
    }

    let bootnodes = get_bootnodes(opts, network, data_dir);

    let context = P2PContext::new(
        local_p2p_node,
        local_node_record,
        tracker.clone(),
        signer,
        peer_table.clone(),
        store,
        blockchain,
        get_client_version(),
    );

    context.set_fork_id().await.expect("Set fork id");

    ethrex_p2p::start_network(context, bootnodes)
        .await
        .expect("Network starts");

    tracker.spawn(ethrex_p2p::periodically_show_peer_stats(peer_table.clone()));
}

pub fn init_metrics(opts: &Options, tracker: TaskTracker) {
    tracing::info!(
        "Starting metrics server on {}:{}",
        opts.metrics_addr,
        opts.metrics_port
    );
    let metrics_api = ethrex_metrics::api::start_prometheus_metrics_api(
        opts.metrics_addr.clone(),
        opts.metrics_port.clone(),
    );
    tracker.spawn(metrics_api);
}

pub fn parse_socket_addr(addr: &str, port: &str) -> io::Result<SocketAddr> {
    // NOTE: this blocks until hostname can be resolved
    format!("{addr}:{port}")
        .to_socket_addrs()?
        .next()
        .ok_or(io::Error::new(
            io::ErrorKind::NotFound,
            "Failed to parse socket address",
        ))
}

pub fn get_local_p2p_node(opts: &Options, signer: &SigningKey) -> Node {
    let udp_socket_addr = parse_socket_addr(&opts.discovery_addr, &opts.discovery_port)
        .expect("Failed to parse discovery address and port");
    let tcp_socket_addr =
        parse_socket_addr(&opts.p2p_addr, &opts.p2p_port).expect("Failed to parse addr and port");

    // TODO: If hhtp.addr is 0.0.0.0 we get the local ip as the one of the node, otherwise we use the provided one.
    // This is fine for now, but we might need to support more options in the future.
    let p2p_node_ip = if udp_socket_addr.ip() == Ipv4Addr::new(0, 0, 0, 0) {
        local_ip().expect("Failed to get local ip")
    } else {
        udp_socket_addr.ip()
    };

    let local_public_key = public_key_from_signing_key(signer);

    let node = Node::new(
        p2p_node_ip,
        udp_socket_addr.port(),
        tcp_socket_addr.port(),
        local_public_key,
    );

    // TODO Find a proper place to show node information
    // https://github.com/lambdaclass/ethrex/issues/836
    let enode = node.enode_url();
    tracing::info!("Node: {enode}");

    node
}

pub fn get_authrpc_socket_addr(opts: &Options) -> SocketAddr {
    parse_socket_addr(&opts.authrpc_addr, &opts.authrpc_port)
        .expect("Failed to parse authrpc address and port")
}

pub fn get_http_socket_addr(opts: &Options) -> SocketAddr {
    parse_socket_addr(&opts.http_addr, &opts.http_port)
        .expect("Failed to parse http address and port")
}

pub fn get_sequencer_socket_addr(full_node_opts: &FullNodeOptions) -> SocketAddr {
    parse_socket_addr(
        &full_node_opts.sequencer_host,
        &full_node_opts.sequencer_port.to_string(),
    )
    .expect("Failed to parse full node address and port")
}

pub fn get_valid_delegation_addresses(opts: &Options) -> Vec<Address> {
    let Some(ref path) = opts.sponsorable_addresses_file_path else {
        tracing::warn!("No valid addresses provided, ethrex_SendTransaction will always fail");
        return Vec::new();
    };
    let addresses: Vec<Address> = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Failed to load file {path}"))
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string().parse::<Address>())
        .filter_map(Result::ok)
        .collect();
    if addresses.is_empty() {
        tracing::warn!("No valid addresses provided, ethrex_SendTransaction will always fail");
    }
    addresses
}

#[allow(clippy::too_many_arguments)]
pub async fn init_full_node_rpc_api(
    opts: &Options,
    full_node_opts: &FullNodeOptions,
    peer_table: Arc<Mutex<KademliaTable>>,
    local_p2p_node: Node,
    local_node_record: NodeRecord,
    store: Store,
    blockchain: Arc<Blockchain>,
    cancel_token: CancellationToken,
    tracker: TaskTracker,
    rollup_store: StoreRollup,
) {
    let peer_handler = PeerHandler::new(peer_table);

    // Create SyncManager
    let syncer = SyncManager::new(
        peer_handler.clone(),
        opts.syncmode.clone(),
        cancel_token,
        blockchain.clone(),
        store.clone(),
    )
    .await;

    let http_addr = get_http_socket_addr(opts);
    let sequencer_addr = get_sequencer_socket_addr(full_node_opts);
    let authrpc_addr = get_authrpc_socket_addr(opts);
    let jwt_secret = read_jwtsecret_file(&opts.authrpc_jwtsecret);
    let client_version = get_client_version();

    let url = format!("http://{sequencer_addr}");
    // Create MojaveClient
    let mojave_client = Client::new(vec![&url]).expect("unable to init sync client");
    // Create EthClient
    let eth_client = EthClient::new(&url).expect("unable to init eth client");

    let rpc_api = mojave_networking::rpc::full_node::start_api(
        http_addr,
        authrpc_addr,
        store,
        blockchain,
        jwt_secret,
        local_p2p_node,
        local_node_record,
        syncer,
        peer_handler,
        client_version,
        rollup_store,
        mojave_client,
        eth_client,
    );

    tracker.spawn(rpc_api);
}

#[allow(clippy::too_many_arguments)]
pub async fn init_sequencer_rpc_api(
    opts: &Options,
    sequencer_opts: &SequencerOpts,
    peer_table: Arc<Mutex<KademliaTable>>,
    local_p2p_node: Node,
    local_node_record: NodeRecord,
    store: Store,
    blockchain: Arc<Blockchain>,
    cancel_token: CancellationToken,
    tracker: TaskTracker,
    rollup_store: StoreRollup,
) {
    let peer_handler = PeerHandler::new(peer_table);

    // Create SyncManager
    let syncer = SyncManager::new(
        peer_handler.clone(),
        opts.syncmode.clone(),
        cancel_token,
        blockchain.clone(),
        store.clone(),
    )
    .await;

    let http_addr = get_http_socket_addr(opts);
    let authrpc_addr = get_authrpc_socket_addr(opts);
    let jwt_secret = read_jwtsecret_file(&opts.authrpc_jwtsecret);
    let client_version = get_client_version();

    // Create MojaveClient
    let addrs: Vec<String> = sequencer_opts
        .full_node_addresses
        .iter()
        .map(|addr| format!("http://{addr}"))
        .collect();
    let addrs = addrs.iter().map(|addr| addr.as_str()).collect();
    let mojave_client = Client::new(addrs).expect("unable to init sync client");

    let rpc_api = mojave_networking::rpc::sequencer::start_api(
        http_addr,
        authrpc_addr,
        store,
        blockchain,
        jwt_secret,
        local_p2p_node,
        local_node_record,
        syncer,
        peer_handler,
        client_version,
        rollup_store,
        mojave_client,
    );

    tracker.spawn(rpc_api);
}

pub async fn init_rollup_store(data_dir: &str) -> StoreRollup {
    cfg_if::cfg_if! {
        if #[cfg(feature = "sql")] {
            let engine_type = EngineTypeRollup::SQL;
        } else if #[cfg(feature = "redb")] {
            let engine_type = EngineTypeRollup::RedB;
        } else if #[cfg(feature = "libmdbx")] {
            let engine_type = EngineTypeRollup::Libmdbx;
        } else {
            let engine_type = EngineTypeRollup::InMemory;
        }
    }
    let rollup_store =
        StoreRollup::new(data_dir, engine_type).expect("Failed to create StoreRollup");
    rollup_store
        .init()
        .await
        .expect("Failed to init rollup store");
    rollup_store
}
