use clap::{ArgAction, Parser, Subcommand};
use mojave_node_lib::types::{Node, SyncMode};
use mojave_utils::network::Network;
use tracing::Level;

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

    #[arg(
    	long = "bootnodes",
     	value_parser = clap::value_parser!(Node),
      	value_name = "BOOTNODE_LIST",
       	value_delimiter = ',',
        num_args = 1..,
        help = "Comma separated enode URLs for P2P discovery bootstrap.",
        help_heading = "P2P options"
    )]
    pub bootnodes: Vec<Node>,

    #[arg(
        long = "syncmode",
        value_enum,
        value_name = "SYNC_MODE",
        help = "The way in which the node will sync its state.",
        long_help = "Can be either \"full\" or \"snap\" with \"full\" as default value.",
        help_heading = "P2P options"
    )]
    pub syncmode: Option<SyncMode>,

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
        default_value = ".mojave/mojave-node",
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

impl From<&Options> for mojave_node_lib::types::NodeOptions {
    fn from(options: &Options) -> Self {
        Self {
            http_addr: options.http_addr.clone(),
            http_port: options.http_port.clone(),
            authrpc_addr: options.authrpc_addr.clone(),
            authrpc_port: options.authrpc_port.clone(),
            authrpc_jwtsecret: options.authrpc_jwtsecret.clone(),
            p2p_enabled: options.p2p_enabled,
            p2p_addr: options.p2p_addr.clone(),
            p2p_port: options.p2p_port.clone(),
            discovery_addr: options.discovery_addr.clone(),
            discovery_port: options.discovery_port.clone(),
            network: options.network.clone(),
            bootnodes: options.bootnodes.clone(),
            datadir: options.datadir.clone(),
            syncmode: options.syncmode.unwrap_or(SyncMode::Full),
            sponsorable_addresses_file_path: options.sponsorable_addresses_file_path.clone(),
            metrics_addr: options.metrics_addr.clone(),
            metrics_port: options.metrics_port.clone(),
            metrics_enabled: options.metrics_enabled,
            force: options.force,
        }
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Parser)]
#[command(
    name = "mojave-node",
    author,
    version,
    about = "mojave-node is the node implementation for the Mojave network.",
    arg_required_else_help = true
)]
pub struct Cli {
    #[arg(
        long = "log.level",
        value_name = "LOG_LEVEL",
        help = "The verbosity level used for logs.",
        long_help = "Possible values: info, debug, trace, warn, error",
        help_heading = "Node options"
    )]
    pub log_level: Option<Level>,
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn run() -> Self {
        Self::parse()
    }
}

#[derive(Subcommand)]
pub enum Command {
    #[command(name = "init", about = "Run the node")]
    Start {
        #[command(flatten)]
        options: Options,
    },
}
