use crate::{
    network::{MAINNET_BOOTNODES, Network, TESTNET_BOOTNODES},
    options::Options,
};
use ethrex::utils::read_node_config_file;
use ethrex_common::Address;
use ethrex_p2p::{network::public_key_from_signing_key, types::Node};
use ethrex_storage_rollup::{EngineTypeRollup, StoreRollup};
use secp256k1::SecretKey;
use std::{
    fs, io,
    net::{Ipv4Addr, SocketAddr, ToSocketAddrs},
    path::PathBuf,
};

pub fn resolve_data_dir(data_dir: &str) -> String {
    let path = match std::env::home_dir() {
        Some(home) => home.join(data_dir),
        None => PathBuf::from(".").join(data_dir),
    };

    // Create the directory in full recursion.
    if !path.exists() {
        std::fs::create_dir_all(&path).expect("Failed to create the data directory.");
    }

    path.to_str()
        .expect("Invalid UTF-8 in data directory")
        .to_owned()
}

pub fn get_bootnodes(opts: &Options, network: &Network, data_dir: &str) -> Vec<Node> {
    let mut bootnodes: Vec<Node> = opts.bootnodes.clone();

    match network {
        Network::Mainnet => {
            tracing::info!("Adding mainnet preset bootnodes");
            bootnodes.extend(MAINNET_BOOTNODES.clone());
        }
        Network::Testnet => {
            tracing::info!("Adding testnet preset bootnodes");
            bootnodes.extend(TESTNET_BOOTNODES.clone());
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

pub fn get_http_socket_addr(opts: &Options) -> SocketAddr {
    parse_socket_addr(&opts.http_addr, &opts.http_port)
        .expect("Failed to parse http address and port")
}

pub fn get_authrpc_socket_addr(opts: &Options) -> SocketAddr {
    parse_socket_addr(&opts.authrpc_addr, &opts.authrpc_port)
        .expect("Failed to parse authrpc address and port")
}

pub fn get_local_p2p_node(opts: &Options, signer: &SecretKey) -> Node {
    let udp_socket_addr = parse_socket_addr(&opts.discovery_addr, &opts.discovery_port)
        .expect("Failed to parse discovery address and port");
    let tcp_socket_addr =
        parse_socket_addr(&opts.p2p_addr, &opts.p2p_port).expect("Failed to parse addr and port");

    // TODO: If hhtp.addr is 0.0.0.0 we get the local ip as the one of the node, otherwise we use the provided one.
    // This is fine for now, but we might need to support more options in the future.
    let p2p_node_ip = if udp_socket_addr.ip() == Ipv4Addr::new(0, 0, 0, 0) {
        local_ip_address::local_ip().expect("Failed to get local ip")
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

pub async fn init_rollup_store(data_dir: &str, engine_type: EngineTypeRollup) -> StoreRollup {
    let rollup_store =
        StoreRollup::new(data_dir, engine_type).expect("Failed to create StoreRollup");
    rollup_store
        .init()
        .await
        .expect("Failed to init rollup store");
    rollup_store
}
