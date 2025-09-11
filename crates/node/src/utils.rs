use crate::{
    error::{Error, Result},
    types::NodeConfigFile,
};
use bytes::Bytes;
use ethrex_p2p::{
    kademlia::Kademlia,
    types::{Node, NodeRecord},
    utils::public_key_from_signing_key,
};
use mojave_utils::network::{MAINNET_BOOTNODES, Network, TESTNET_BOOTNODES};
use secp256k1::SecretKey;
use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};
use tracing::{error, info};

impl NodeConfigFile {
    pub async fn new(table: Kademlia, node_record: NodeRecord) -> Self {
        let mut connected_peers = vec![];

        for (_, peer) in table.peers.lock().await.iter() {
            connected_peers.push(peer.node.clone());
        }
        NodeConfigFile {
            known_peers: connected_peers,
            node_record,
        }
    }
}

pub fn read_node_config_file(file_path: PathBuf) -> Result<NodeConfigFile> {
    match std::fs::File::open(file_path) {
        Ok(file) => serde_json::from_reader(file).map_err(Error::SerdeJson),
        Err(e) => Err(Error::Custom(format!("No config file found: {e}"))),
    }
}

pub async fn read_node_config_file_async(file_path: PathBuf) -> Result<NodeConfigFile> {
    match tokio::fs::read(file_path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(Error::SerdeJson),
        Err(e) => Err(Error::Custom(format!("No config file found: {e}"))),
    }
}

pub async fn store_node_config_file(config: NodeConfigFile, file_path: PathBuf) {
    let json = match serde_json::to_string(&config) {
        Ok(json) => json,
        Err(e) => {
            error!("Could not store config in file: {e:?}");
            return;
        }
    };

    if let Err(e) = tokio::fs::write(file_path, json).await {
        error!("Could not store config in file: {e:?}");
    };
}

pub fn jwtsecret_from_bytes(bytes: &[u8]) -> Result<Bytes> {
    let mut contents = String::from_utf8_lossy(bytes).to_string();
    if contents.starts_with("0x") {
        contents = contents[2..].to_string();
    }
    contents = contents.trim_end_matches('\n').to_string();
    Ok(Bytes::from(hex::decode(contents)?))
}

pub async fn read_jwtsecret_file(jwt_secret_path: &str) -> Result<Bytes> {
    match tokio::fs::read(jwt_secret_path).await {
        Ok(bytes) => jwtsecret_from_bytes(&bytes),
        Err(_) => write_jwtsecret_file(jwt_secret_path).await,
    }
}

pub async fn write_jwtsecret_file(jwt_secret_path: &str) -> Result<Bytes> {
    info!("JWT secret not found in the provided path, generating JWT secret");
    let secret = generate_jwt_secret();
    tokio::fs::write(jwt_secret_path, &secret).await?;
    Ok(Bytes::from(hex::decode(secret)?))
}

pub fn generate_jwt_secret() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut secret = [0u8; 32];
    rng.fill(&mut secret);
    hex::encode(secret)
}

pub async fn resolve_data_dir(data_dir: &str) -> Result<String> {
    let path = match std::env::home_dir() {
        Some(home) => home.join(data_dir),
        None => PathBuf::from(".").join(data_dir),
    };

    // Create the directory in full recursion.
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let s = path
        .to_str()
        .ok_or_else(|| Error::Custom("Invalid UTF-8 in data directory".to_string()))?;
    Ok(s.to_owned())
}

pub async fn get_bootnodes(bootnodes: Vec<Node>, network: &Network, data_dir: &str) -> Vec<Node> {
    let mut bootnodes: Vec<Node> = bootnodes.clone();

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

    match read_node_config_file_async(config_file).await {
        Ok(ref mut config) => bootnodes.append(&mut config.known_peers),
        Err(e) => tracing::error!("Could not read from peers file: {e}"),
    };

    bootnodes
}

pub async fn parse_socket_addr(addr: &str, port: &str) -> Result<SocketAddr> {
    let mut addrs = tokio::net::lookup_host(format!("{addr}:{port}")).await?;
    addrs
        .next()
        .ok_or_else(|| Error::Custom(format!("Could not resolve address: {addr}:{port}")))
}

pub async fn get_http_socket_addr(http_addr: &str, http_port: &str) -> Result<SocketAddr> {
    parse_socket_addr(http_addr, http_port).await
}

pub async fn get_authrpc_socket_addr(authrpc_addr: &str, authrpc_port: &str) -> Result<SocketAddr> {
    parse_socket_addr(authrpc_addr, authrpc_port).await
}

pub async fn get_local_p2p_node(
    discovery_addr: &str,
    discovery_port: &str,
    p2p_addr: &str,
    p2p_port: &str,
    signer: &SecretKey,
) -> Result<Node> {
    let udp_socket_addr = parse_socket_addr(discovery_addr, discovery_port).await?;
    let tcp_socket_addr = parse_socket_addr(p2p_addr, p2p_port).await?;

    // TODO: If hhtp.addr is 0.0.0.0 we get the local ip as the one of the node, otherwise we use the provided one.
    // This is fine for now, but we might need to support more options in the future.
    let p2p_node_ip = if udp_socket_addr.ip() == Ipv4Addr::new(0, 0, 0, 0) {
        local_ip_address::local_ip()
    } else {
        Ok(udp_socket_addr.ip())
    }?;

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

    Ok(node)
}
