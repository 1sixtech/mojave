use bytes::Bytes;
use ethrex_common::Address;
use ethrex_p2p::{
    kademlia::KademliaTable,
    network::public_key_from_signing_key,
    types::{Node, NodeRecord},
};
use ethrex_storage_rollup::{EngineTypeRollup, StoreRollup};
use mojave_utils::network::{MAINNET_BOOTNODES, Network, TESTNET_BOOTNODES};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    fs::File,
    io,
    io::Read as _,
    net::{Ipv4Addr, SocketAddr, ToSocketAddrs},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::Mutex;
use tracing::{error, info};

#[derive(Serialize, Deserialize)]
pub struct NodeConfigFile {
    pub known_peers: Vec<Node>,
    pub node_record: NodeRecord,
}

impl NodeConfigFile {
    pub async fn new(table: Arc<Mutex<KademliaTable>>, node_record: NodeRecord) -> Self {
        let mut connected_peers = vec![];

        for peer in table.lock().await.iter_peers() {
            if peer.is_connected {
                connected_peers.push(peer.node.clone());
            }
        }
        NodeConfigFile {
            known_peers: connected_peers,
            node_record,
        }
    }
}

pub fn read_node_config_file(file_path: PathBuf) -> Result<NodeConfigFile, String> {
    match std::fs::File::open(file_path) {
        Ok(file) => {
            serde_json::from_reader(file).map_err(|e| format!("Invalid node config file {e}"))
        }
        Err(e) => Err(format!("No config file found: {e}")),
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

    if let Err(e) = std::fs::write(file_path, json) {
        error!("Could not store config in file: {e:?}");
    };
}

pub fn jwtsecret_file(file: &mut File) -> Bytes {
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Failed to read jwt secret file");
    if contents[0..2] == *"0x" {
        contents = contents[2..contents.len()].to_string();
    }
    contents = contents.trim_end_matches('\n').to_string();
    hex::decode(contents)
        .expect("Secret should be hex encoded")
        .into()
}

pub fn read_jwtsecret_file(jwt_secret_path: &str) -> Bytes {
    match File::open(jwt_secret_path) {
        Ok(mut file) => jwtsecret_file(&mut file),
        Err(_) => write_jwtsecret_file(jwt_secret_path),
    }
}

pub fn write_jwtsecret_file(jwt_secret_path: &str) -> Bytes {
    info!("JWT secret not found in the provided path, generating JWT secret");
    let secret = generate_jwt_secret();
    std::fs::write(jwt_secret_path, &secret).expect("Unable to write JWT secret file");
    hex::decode(secret)
        .map(Bytes::from)
        .expect("Failed to decode generated JWT secret")
}

pub fn generate_jwt_secret() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut secret = [0u8; 32];
    rng.fill(&mut secret);
    hex::encode(secret)
}

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

pub fn get_bootnodes(bootnodes: Vec<Node>, network: &Network, data_dir: &str) -> Vec<Node> {
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

pub fn get_http_socket_addr(http_addr: &str, http_port: &str) -> SocketAddr {
    parse_socket_addr(http_addr, http_port).expect("Failed to parse http address and port")
}

pub fn get_authrpc_socket_addr(authrpc_addr: &str, authrpc_port: &str) -> SocketAddr {
    parse_socket_addr(authrpc_addr, authrpc_port).expect("Failed to parse authrpc address and port")
}

pub fn get_local_p2p_node(
    discovery_addr: &str,
    discovery_port: &str,
    p2p_addr: &str,
    p2p_port: &str,
    signer: &SecretKey,
) -> Node {
    let udp_socket_addr = parse_socket_addr(discovery_addr, discovery_port)
        .expect("Failed to parse discovery address and port");
    let tcp_socket_addr =
        parse_socket_addr(p2p_addr, p2p_port).expect("Failed to parse addr and port");

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

pub fn get_valid_delegation_addresses(
    sponsorable_addresses_file_path: Option<String>,
) -> Vec<Address> {
    let Some(ref path) = sponsorable_addresses_file_path else {
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
