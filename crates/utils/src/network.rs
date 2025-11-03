use std::{
    fmt,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use ethrex_common::types::{Genesis, GenesisError};
use ethrex_p2p::types::Node;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use crate::error::{NetworkError as Error, NetworkResult as Result};

pub const TESTNET_GENESIS_PATH: &str = "data/testnet-genesis.json";
// Just a placeholder for now, will be replaced with real file later
const TESTNET_BOOTNODES_PATH: &str = "cmd/mojave/networks/testnet/bootnodes.json";

pub const MAINNET_GENESIS_PATH: &str = "cmd/mojave/networks/mainnet/genesis.json";
const MAINNET_BOOTNODES_PATH: &str = "cmd/mojave/networks/mainnet/bootnodes.json";

fn read_bootnodes(path: &str) -> Vec<Node> {
    // ethrex_p2p::rlpx::Message
    std::fs::File::open(path)
        .map_err(|e| {
            tracing::warn!(path, error = %e, "Failed to open bootnodes file; using empty list");
        })
        .and_then(|file| {
            serde_json::from_reader(file).map_err(|e| {
                tracing::warn!(path, error = %e, "Failed to parse bootnodes file; using empty list");
            })
        })
        .unwrap_or_default()
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
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => Err(Error::Custom(format!(
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
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => Err(Error::Custom(format!(
            "UDP port {} already in use at {}",
            socket_addr.port(),
            socket_addr.ip()
        ))),
        Err(e) => Err(Error::Io(e)),
    }
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

lazy_static! {
    pub static ref MAINNET_BOOTNODES: Vec<Node> = read_bootnodes(MAINNET_BOOTNODES_PATH);
    pub static ref TESTNET_BOOTNODES: Vec<Node> = read_bootnodes(TESTNET_BOOTNODES_PATH);
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum Network {
    #[default]
    DefaultNet,
    Mainnet,
    Testnet,
    GenesisPath(PathBuf),
}

impl From<&str> for Network {
    fn from(value: &str) -> Self {
        match value {
            "default" => Network::DefaultNet,
            "mainnet" => Network::Mainnet,
            "testnet" => Network::Testnet,
            s => Network::GenesisPath(PathBuf::from(s)),
        }
    }
}

impl From<PathBuf> for Network {
    fn from(value: PathBuf) -> Self {
        Network::GenesisPath(value)
    }
}

impl Network {
    pub fn get_genesis_path(&self) -> &Path {
        match self {
            Network::DefaultNet => {
                // should never happen, but just in case
                panic!("DefaultNet does not have a genesis path");
            }
            Network::Mainnet => Path::new(MAINNET_GENESIS_PATH),
            Network::Testnet => Path::new(TESTNET_GENESIS_PATH),
            Network::GenesisPath(s) => s,
        }
    }
    pub fn get_genesis(&self) -> core::result::Result<Genesis, GenesisError> {
        // If DefaultNet, construct a default genesis
        if let Network::DefaultNet = self {
            return Ok(Genesis::default());
        }
        Genesis::try_from(self.get_genesis_path())
    }

    pub fn get_bootnodes(&self) -> Vec<Node> {
        match self {
            Network::Mainnet => MAINNET_BOOTNODES.clone(),
            Network::Testnet => TESTNET_BOOTNODES.clone(),
            Network::DefaultNet | Network::GenesisPath(_) => Vec::new(),
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Network::DefaultNet => write!(f, "default"),
            Network::Mainnet => write!(f, "mainnet"),
            Network::Testnet => write!(f, "testnet"),
            Network::GenesisPath(path) => write!(f, "{path:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn from_str_variants_map_correctly() {
        assert!(matches!(Network::from("default"), Network::DefaultNet));
        assert!(matches!(Network::from("mainnet"), Network::Mainnet));
        assert!(matches!(Network::from("testnet"), Network::Testnet));

        let network = Network::from("/tmp/genesis.json");
        match network {
            Network::GenesisPath(p) => assert_eq!(p, PathBuf::from("/tmp/genesis.json")),
            _ => panic!("expected GenesisPath"),
        }
    }

    #[test]
    fn from_pathbuf_becomes_genesispath() {
        let pathbuf = PathBuf::from("tmp/genesis.json");
        let network = Network::from(pathbuf.clone());
        match network {
            Network::GenesisPath(p) => assert_eq!(p, pathbuf),
            _ => panic!("expected GenesisPath"),
        }
    }

    #[test]
    fn display_formats_are_stable() {
        assert_eq!(format!("{}", Network::DefaultNet), "default");
        assert_eq!(format!("{}", Network::Mainnet), "mainnet");
        assert_eq!(format!("{}", Network::Testnet), "testnet");

        let network = Network::from("1six/mojave.json");
        let s = format!("{network}");
        assert!(s.contains("1six/mojave.json"));
    }

    #[test]
    #[should_panic(expected = "DefaultNet does not have a genesis path")]
    fn defaultnet_get_genesis_path_panics() {
        let _ = Network::DefaultNet.get_genesis_path();
    }

    #[test]
    fn invalid_path_get_genesis_err() {
        let network = Network::from("/does/not/exist.json");
        let err = network.get_genesis().unwrap_err();

        assert!(matches!(
            err,
            GenesisError::File(ref e) if e.kind() == std::io::ErrorKind::NotFound
        ));
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
}
