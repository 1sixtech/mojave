use crate::network::Network;
use clap::{ArgAction, Parser};
use ethrex::utils;
use ethrex_p2p::{sync::SyncMode, types::Node};
use std::fmt;

#[derive(Parser)]
pub struct Options {
    #[arg(
        long = "network",
        default_value_t = Network::default(),
        value_name = "GENESIS_FILE_PATH",
        help = "Receives a `Genesis` struct in json format. This is the only argument which is required. You can look at some example genesis files at `test_data/genesis*`.",
        long_help = "Alternatively, the name of a known network can be provided instead to use its preset genesis file and include its preset bootnodes. The networks currently supported include holesky, sepolia, hoodi and mainnet.",
        help_heading = "Node options",
        env = "ETHREX_NETWORK",
        value_parser = clap::value_parser!(Network),
    )]
    pub network: Network,

    #[arg(long = "bootnodes", value_parser = clap::value_parser!(Node), value_name = "BOOTNODE_LIST", value_delimiter = ',', num_args = 1.., help = "Comma separated enode URLs for P2P discovery bootstrap.", help_heading = "P2P options")]
    pub bootnodes: Vec<Node>,

    #[arg(long = "syncmode", default_value = "full", value_name = "SYNC_MODE", value_parser = utils::parse_sync_mode, help = "The way in which the node will sync its state.", long_help = "Can be either \"full\" or \"snap\" with \"full\" as default value.", help_heading = "P2P options")]
    pub syncmode: SyncMode,

    #[arg(
        long = "sponsorable-addresses",
        value_name = "SPONSORABLE_ADDRESSES_PATH",
        help = "Path to a file containing addresses of contracts to which ethrex_SendTransaction should sponsor txs",
        help_heading = "L2 options"
    )]
    pub sponsorable_addresses_file_path: Option<String>,

    #[arg(
        long = "datadir",
        value_name = "DATABASE_DIRECTORY",
        help = "If the datadir is the word `memory`, ethrex will use the InMemory Engine",
        default_value = "mojave",
        help = "Receives the name of the directory where the Database is located.",
        long_help = "If the datadir is the word `memory`, ethrex will use the `InMemory Engine`.",
        help_heading = "Node options",
        env = "ETHREX_DATADIR"
    )]
    pub datadir: String,

    #[arg(
        long = "force",
        help = "Force remove the database",
        long_help = "Delete the database without confirmation.",
        action = clap::ArgAction::SetTrue,
        help_heading = "Node options"
    )]
    pub force: bool,

    #[arg(
        long = "metrics.addr",
        value_name = "ADDRESS",
        default_value = "0.0.0.0",
        help_heading = "Node options"
    )]
    pub metrics_addr: String,

    #[arg(
        long = "metrics.port",
        value_name = "PROMETHEUS_METRICS_PORT",
        default_value = "9090", // Default Prometheus port (https://prometheus.io/docs/tutorials/getting_started/#show-me-how-it-is-done).
        help_heading = "Node options",
        env = "ETHREX_METRICS_PORT"
    )]
    pub metrics_port: String,

    #[arg(
        long = "metrics",
        action = ArgAction::SetTrue,
        help = "Enable metrics collection and exposition",
        help_heading = "Node options"
    )]
    pub metrics_enabled: bool,

    #[arg(
        long = "http.addr",
        default_value = "0.0.0.0",
        value_name = "ADDRESS",
        help = "Listening address for the http rpc server.",
        help_heading = "RPC options",
        env = "ETHREX_HTTP_ADDR"
    )]
    pub http_addr: String,

    #[arg(
        long = "http.port",
        default_value = "8545",
        value_name = "PORT",
        help = "Listening port for the http rpc server.",
        help_heading = "RPC options",
        env = "ETHREX_HTTP_PORT"
    )]
    pub http_port: String,

    #[arg(
        long = "authrpc.addr",
        default_value = "localhost",
        value_name = "ADDRESS",
        help = "Listening address for the authenticated rpc server.",
        help_heading = "RPC options"
    )]
    pub authrpc_addr: String,

    #[arg(
        long = "authrpc.port",
        default_value = "8551",
        value_name = "PORT",
        help = "Listening port for the authenticated rpc server.",
        help_heading = "RPC options"
    )]
    pub authrpc_port: String,

    #[arg(
        long = "authrpc.jwtsecret",
        default_value = "jwt.hex",
        value_name = "JWTSECRET_PATH",
        help = "Receives the jwt secret used for authenticated rpc requests.",
        help_heading = "RPC options"
    )]
    pub authrpc_jwtsecret: String,

    #[arg(long = "p2p.enabled", default_value =  "true" , value_name = "P2P_ENABLED", action = ArgAction::SetTrue, help_heading = "P2P options")]
    pub p2p_enabled: bool,

    #[arg(
        long = "p2p.addr",
        default_value = "0.0.0.0",
        value_name = "ADDRESS",
        help_heading = "P2P options"
    )]
    pub p2p_addr: String,

    #[arg(
        long = "p2p.port",
        default_value = "30303",
        value_name = "PORT",
        help_heading = "P2P options"
    )]
    pub p2p_port: String,

    #[arg(
        long = "discovery.addr",
        default_value = "0.0.0.0",
        value_name = "ADDRESS",
        help = "UDP address for P2P discovery.",
        help_heading = "P2P options"
    )]
    pub discovery_addr: String,

    #[arg(
        long = "discovery.port",
        default_value = "30303",
        value_name = "PORT",
        help = "UDP port for P2P discovery.",
        help_heading = "P2P options"
    )]
    pub discovery_port: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            http_addr: Default::default(),
            http_port: Default::default(),
            authrpc_addr: Default::default(),
            authrpc_port: Default::default(),
            authrpc_jwtsecret: Default::default(),
            p2p_enabled: Default::default(),
            p2p_addr: Default::default(),
            p2p_port: Default::default(),
            discovery_addr: Default::default(),
            discovery_port: Default::default(),
            network: Network::Mainnet,
            bootnodes: Default::default(),
            datadir: Default::default(),
            syncmode: Default::default(),
            sponsorable_addresses_file_path: None,
            metrics_addr: "0.0.0.0".to_owned(),
            metrics_port: Default::default(),
            metrics_enabled: Default::default(),
            force: false,
        }
    }
}

impl fmt::Debug for Options {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Options")
            .field("network", &self.network)
            .field("bootnodes", &self.bootnodes)
            .field("datadir", &self.datadir)
            .field("force", &self.force)
            .field("syncmode", &self.syncmode)
            .field("metrics_addr", &self.metrics_addr)
            .field("metrics_port", &self.metrics_port)
            .field("metrics_enabled", &self.metrics_enabled)
            .field("http_addr", &self.http_addr)
            .field("http_port", &self.http_port)
            .field("authrpc_addr", &self.authrpc_addr)
            .field("authrpc_port", &self.authrpc_port)
            .field("authrpc_jwtsecret", &self.authrpc_jwtsecret)
            .field("p2p_enabled", &self.p2p_enabled)
            .field("p2p_addr", &self.p2p_addr)
            .field("p2p_port", &self.p2p_port)
            .field("discovery_addr", &self.discovery_addr)
            .field("discovery_port", &self.discovery_port)
            .finish()
    }
}
