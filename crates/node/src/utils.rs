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
use mojave_utils::network::Network;
use secp256k1::SecretKey;
use std::{
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};
use tracing::{error, info};

impl NodeConfigFile {
    pub async fn new(table: Kademlia, node_record: NodeRecord) -> Self {
        let connected_peers: Vec<Node> = table
            .peers
            .lock()
            .await
            .values()
            .map(|p| p.node.clone())
            .collect();

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

pub async fn resolve_data_dir(data_dir: &str) -> Result<(PathBuf, String)> {
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
        .ok_or_else(|| Error::Custom("Invalid UTF-8 in data directory".to_string()))?
        .to_owned();
    Ok((path, s))
}

pub async fn get_bootnodes(
    mut bootnodes: Vec<Node>,
    network: &Network,
    data_dir: &str,
) -> Vec<Node> {
    const NODE_CONFIG_FILE: &str = "node_config.json";
    match network {
        Network::Mainnet => {
            tracing::info!("Adding mainnet preset bootnodes");
            bootnodes.extend(network.get_bootnodes());
        }
        Network::Testnet => {
            tracing::info!("Adding testnet preset bootnodes");
            bootnodes.extend(network.get_bootnodes());
        }
        _ => {}
    }

    if bootnodes.is_empty() {
        tracing::warn!(
            "No bootnodes specified. This node will not be able to connect to the network."
        );
    }

    let config_file = Path::new(data_dir).join(NODE_CONFIG_FILE);

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

/// Ensures a TCP port is available by attempting to bind to it and immediately
/// releasing the socket. Returns Ok(()) if the port can be bound, otherwise
/// returns an Error describing why it is unavailable.
pub async fn ensure_tcp_port_available(addr: &str, port: &str) -> Result<()> {
    let socket_addr = parse_socket_addr(addr, port).await?;

    match tokio::net::TcpListener::bind(socket_addr).await {
        Ok(listener) => {
            drop(listener);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => Err(Error::Config(format!(
            "TCP port {} already in use at {}",
            socket_addr.port(),
            socket_addr.ip()
        ))),
        Err(e) => Err(Error::Io(e)),
    }
}

/// Ensures a UDP port is available by attempting to bind to it and immediately
/// releasing the socket. Returns Ok(()) if the port can be bound, otherwise
/// returns an Error describing why it is unavailable.
pub async fn ensure_udp_port_available(addr: &str, port: &str) -> Result<()> {
    let socket_addr = parse_socket_addr(addr, port).await?;

    match tokio::net::UdpSocket::bind(socket_addr).await {
        Ok(socket) => {
            drop(socket);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => Err(Error::Config(format!(
            "UDP port {} already in use at {}",
            socket_addr.port(),
            socket_addr.ip()
        ))),
        Err(e) => Err(Error::Io(e)),
    }
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

    // TODO: If http.addr is 0.0.0.0 we get the local ip as the one of the node, otherwise we use the provided one.
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

#[cfg(test)]
mod tests {
    use super::*;
    use hex::FromHex;
    use std::{
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tokio::fs;

    // helpers
    fn unique_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{nanos}"))
    }

    // jwt secret / file I/O
    #[test]
    fn jwtsecret_from_bytes_strips_0x_and_newline() {
        // 32bytes(64hex) fixed length
        let hex_str = "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff\n";
        let out = jwtsecret_from_bytes(hex_str.as_bytes()).expect("parse");
        assert_eq!(out.len(), 32);
        assert_eq!(
            hex::encode(&out),
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
        );
    }

    #[test]
    fn generate_jwt_secret_is_64_hex_and_decodes_to_32_bytes() {
        let s = generate_jwt_secret();
        assert_eq!(s.len(), 64, "must be 64 hex chars");
        let bytes = Vec::from_hex(&s).expect("valid hex");
        assert_eq!(bytes.len(), 32);
    }

    #[tokio::test]
    async fn write_jwtsecret_file_creates_file_and_returns_bytes() {
        let path = unique_path("jwt_write_test");
        let path_str = path.to_string_lossy().to_string();

        let bytes = write_jwtsecret_file(&path_str).await.expect("write");
        assert_eq!(bytes.len(), 32);

        let on_disk = fs::read(&path_str).await.expect("exists");
        let text = String::from_utf8(on_disk).expect("utf8");
        assert_eq!(text.trim().len(), 64);

        // cleanup
        let _ = fs::remove_file(&path_str).await;
    }

    #[tokio::test]
    async fn read_jwtsecret_file_generates_when_missing() {
        let path = unique_path("jwt_read_test");
        let path_str = path.to_string_lossy().to_string();

        // If there path is missing, it goes to the auto-generate path
        let bytes = read_jwtsecret_file(&path_str).await.expect("read/generate");
        assert_eq!(bytes.len(), 32);

        // Check if the file actually exists
        assert!(Path::new(&path_str).exists());

        // cleanup
        let _ = fs::remove_file(&path_str).await;
    }

    #[tokio::test]
    async fn resolve_data_dir_creates_parent_dirs_and_returns_full_path_string() {
        // it creates parent dirs only, so use "parent/child" form to test
        let suffix = format!(
            "mojave_ut_parent_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let data_dir = format!("{suffix}/child");

        let (_, full) = resolve_data_dir(&data_dir).await.expect("ok");
        // the full is in the form of $HOME/…/suffix/child
        let full_path = PathBuf::from(&full);
        let parent = full_path.parent().expect("has parent");
        assert!(parent.exists());

        // cleanup
        let _ = fs::remove_dir_all(parent).await;
    }

    #[tokio::test]
    async fn read_node_config_file_async_missing_returns_custom_error() {
        let missing = unique_path("no_config.json");
        let err = read_node_config_file_async(missing).await.unwrap_err();
        let s = format!("{err:?}").to_lowercase();
        assert!(s.contains("no config file"));
    }

    #[test]
    fn read_node_config_file_missing_returns_custom_error() {
        let missing = unique_path("no_sync.json");
        let err = read_node_config_file(missing).unwrap_err();
        let s = format!("{err:?}").to_lowercase();
        assert!(s.contains("no config file"));
    }

    #[tokio::test]
    async fn get_bootnodes_adds_mainnet_presets_when_empty_and_missing_config() {
        let tmp = unique_path("bootnodes_mainnet_dir");
        fs::create_dir_all(&tmp).await.unwrap();

        let out = get_bootnodes(vec![], &Network::Mainnet, tmp.to_str().unwrap()).await;
        // depend on preset being at least 1

        assert!(out.len() >= mojave_utils::network::MAINNET_BOOTNODES.len());

        let _ = fs::remove_dir_all(&tmp).await;
    }

    #[tokio::test]
    async fn get_bootnodes_returns_input_when_dev_and_no_config() {
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let pub_key = public_key_from_signing_key(&secret_key);
        let node = Node::new("127.0.0.1".parse().unwrap(), 30304, 30305, pub_key);

        let tmp = unique_path("bootnodes_dev_dir");
        fs::create_dir_all(&tmp).await.unwrap();

        let out = get_bootnodes(
            vec![node.clone()],
            &Network::DefaultNet,
            tmp.to_str().unwrap(),
        )
        .await;
        assert_eq!(out.len(), 1);

        let _ = fs::remove_dir_all(&tmp).await;
    }

    #[tokio::test]
    async fn parse_socket_addr_ok_and_helpers_delegate() {
        let socket_addr1 = parse_socket_addr("127.0.0.1", "18123").await.unwrap();
        assert_eq!(socket_addr1.port(), 18123);

        let socket_addr2 = get_http_socket_addr("localhost", "18124").await.unwrap();
        assert_eq!(socket_addr2.port(), 18124);

        let socket_addr3 = get_authrpc_socket_addr("127.0.0.1", "18125").await.unwrap();
        assert_eq!(socket_addr3.port(), 18125);
    }

    #[tokio::test]
    async fn parse_socket_addr_invalid_host_errors() {
        let err = parse_socket_addr("invalid.domain.com", "80")
            .await
            .unwrap_err();

        let s = format!("{err:?}").to_lowercase();
        assert!(s.contains("could not") || s.contains("failed") || s.contains("resolve"));
    }

    #[tokio::test]
    async fn ensure_tcp_port_available_returns_ok_for_ephemeral() {
        // Binding to port 0 lets the OS choose a free port; this should always succeed
        ensure_tcp_port_available("127.0.0.1", "0")
            .await
            .expect("port 0 should be bindable");
    }

    #[tokio::test]
    async fn ensure_tcp_port_available_errors_when_taken() {
        // First bind a listener to reserve a real port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind first listener");
        let port = listener.local_addr().expect("local addr").port();

        // Now validating availability should fail
        let err = ensure_tcp_port_available("127.0.0.1", &port.to_string())
            .await
            .expect_err("should detect port in use");

        let s = format!("{err:?}").to_lowercase();
        assert!(s.contains("already in use") || s.contains("in use"));

        // drop listener to cleanup
        drop(listener);
    }

    #[tokio::test]
    async fn ensure_udp_port_available_returns_ok_for_ephemeral() {
        ensure_udp_port_available("127.0.0.1", "0")
            .await
            .expect("port 0 should be bindable for UDP");
    }

    #[tokio::test]
    async fn ensure_udp_port_available_errors_when_taken() {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("bind first udp socket");
        let port = socket.local_addr().expect("local addr").port();

        let err = ensure_udp_port_available("127.0.0.1", &port.to_string())
            .await
            .expect_err("should detect UDP port in use");

        let s = format!("{err:?}").to_lowercase();
        assert!(s.contains("already in use") || s.contains("in use"));

        drop(socket);
    }

    #[tokio::test]
    async fn get_local_p2p_node_uses_local_ip_when_discovery_is_0_0_0_0() {
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let node = get_local_p2p_node("0.0.0.0", "30304", "127.0.0.1", "30305", &secret_key)
            .await
            .unwrap();

        let enode = node.enode_url();
        assert!(enode.contains(":30305"));
    }

    #[tokio::test]
    async fn get_local_p2p_node_uses_given_ip_when_discovery_is_specific() {
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let node = get_local_p2p_node("127.0.0.1", "30310", "127.0.0.1", "30311", &secret_key)
            .await
            .unwrap();

        let enode = node.enode_url();
        assert!(enode.contains(":30311"));
    }
}
