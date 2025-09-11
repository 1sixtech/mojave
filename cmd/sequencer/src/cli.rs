use clap::{ArgAction, ArgGroup, Parser, Subcommand};
use mojave_node_lib::types::{Node, SyncMode};
use mojave_utils::network::Network;

use serde::{Deserialize, Serialize};

#[derive(Parser, Debug, Serialize, Deserialize)]
pub struct Options {
    #[arg(
        long = "network",
        value_name = "GENESIS_FILE_PATH",
        help = "Receives a `Genesis` struct in json format. This is the only argument which is required. You can look at some example genesis files at `test_data/genesis*`.",
        long_help = "Alternatively, the name of a known network can be provided instead to use its preset genesis file and include its preset bootnodes. The networks currently supported include holesky, sepolia, hoodi and mainnet.",
        help_heading = "Node options",
        env = "ETHREX_NETWORK",
        value_parser = clap::value_parser!(Network),
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub network: Option<Network>,

    #[arg(
    	long = "bootnodes",
     	value_parser = clap::value_parser!(Node),
      	value_name = "BOOTNODE_LIST",
       	value_delimiter = ',',
        num_args = 1..,
        help = "Comma separated enode URLs for P2P discovery bootstrap.",
        help_heading = "P2P options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub bootnodes: Option<Vec<Node>>,

    #[arg(
        long = "syncmode",
        value_enum,
        value_name = "SYNC_MODE",
        help = "The way in which the node will sync its state.",
        long_help = "Can be either \"full\" or \"snap\" with \"full\" as default value.",
        help_heading = "P2P options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub syncmode: Option<SyncMode>,

    #[arg(
        long = "sponsorable-addresses",
        value_name = "SPONSORABLE_ADDRESSES_PATH",
        help = "Path to a file containing addresses of contracts to which ethrex_SendTransaction should sponsor txs",
        help_heading = "L2 options"
    )]
    pub sponsorable_addresses_file_path: Option<String>,

    #[arg(
        long = "force",
        help = "Force remove the database",
        long_help = "Delete the database without confirmation.",
        action = clap::ArgAction::SetTrue,
        help_heading = "Node options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub force: Option<bool>,

    #[arg(
        long = "metrics.addr",
        value_name = "ADDRESS",
        help_heading = "Node options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub metrics_addr: Option<String>,

    #[arg(
        long = "metrics.port",
        value_name = "PROMETHEUS_METRICS_PORT",
        // default_value = "9090", // Default Prometheus port (https://prometheus.io/docs/tutorials/getting_started/#show-me-how-it-is-done).
        help_heading = "Node options",
        env = "ETHREX_METRICS_PORT"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub metrics_port: Option<String>,

    #[arg(
        long = "metrics",
        action = ArgAction::SetTrue,
        help = "Enable metrics collection and exposition",
        help_heading = "Node options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub metrics_enabled: Option<bool>,

    #[arg(
        long = "http.addr",
        value_name = "ADDRESS",
        help = "Listening address for the http rpc server.",
        help_heading = "RPC options",
        env = "ETHREX_HTTP_ADDR"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub http_addr: Option<String>,

    #[arg(
        long = "http.port",
        value_name = "PORT",
        help = "Listening port for the http rpc server.",
        help_heading = "RPC options",
        env = "ETHREX_HTTP_PORT"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub http_port: Option<String>,

    #[arg(
        long = "authrpc.addr",
        value_name = "ADDRESS",
        help = "Listening address for the authenticated rpc server.",
        help_heading = "RPC options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub authrpc_addr: Option<String>,

    #[arg(
        long = "authrpc.port",
        value_name = "PORT",
        help = "Listening port for the authenticated rpc server.",
        help_heading = "RPC options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub authrpc_port: Option<String>,

    #[arg(
        long = "authrpc.jwtsecret",
        value_name = "JWTSECRET_PATH",
        help = "Receives the jwt secret used for authenticated rpc requests.",
        help_heading = "RPC options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub authrpc_jwtsecret: Option<String>,

    #[arg(long = "p2p.enabled", value_name = "P2P_ENABLED", action = ArgAction::SetTrue, help_heading = "P2P options")]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub p2p_enabled: Option<bool>,

    #[arg(
        long = "p2p.addr",
        value_name = "ADDRESS",
        help_heading = "P2P options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub p2p_addr: Option<String>,

    #[arg(long = "p2p.port", value_name = "PORT", help_heading = "P2P options")]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub p2p_port: Option<String>,

    #[arg(
        long = "discovery.addr",
        value_name = "ADDRESS",
        help = "UDP address for P2P discovery.",
        help_heading = "P2P options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub discovery_addr: Option<String>,

    #[arg(
        long = "discovery.port",
        value_name = "PORT",
        help = "UDP port for P2P discovery.",
        help_heading = "P2P options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub discovery_port: Option<String>,

    #[arg(
        long = "no-daemon",
        help = "If set, the node will run in the foreground (not as a daemon). By default, the node runs as a daemon.",
        help_heading = "Daemon Options",
        action = clap::ArgAction::SetTrue
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub no_daemon: Option<bool>,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Parser, Serialize, Deserialize, Debug)]
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
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub log_level: Option<String>,
    #[arg(
        long = "datadir",
        value_name = "DATABASE_DIRECTORY",
        help = "If the datadir is the word `memory`, ethrex will use the InMemory Engine",
        default_value = ".mojave/node",
        help = "Receives the name of the directory where the Database is located.",
        long_help = "If the datadir is the word `memory`, ethrex will use the `InMemory Engine`.",
        help_heading = "Node options",
        env = "ETHREX_DATADIR"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub datadir: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn run() -> Self {
        Self::parse()
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub enum Command {
    #[command(name = "init", about = "Run the sequencer")]
    Start {
        #[command(flatten)]
        options: Options,
        #[command(flatten)]
        sequencer_options: SequencerOptions,
    },
    #[command(name = "stop", about = "Stop the sequencer")]
    Stop,
    #[command(name = "get-pub-key", about = "Display the public key of the node")]
    GetPubKey,
}

#[derive(Parser, Serialize, Deserialize)]
#[clap(group(ArgGroup::new("mojave::SequencerOptions")))]
pub struct SequencerOptions {
    #[arg(
        long = "prover.address",
        help = "Allowed domain(s) and port(s) for the prover in the form 'domain:port'",
        help_heading = "Prover Options",
        // default_value = "http://0.0.0.0:3900"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub prover_address: Option<String>,
    #[arg(
        long = "block_time",
        help = "Block creation interval in milliseconds",
        default_value = "1000"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub block_time: Option<u64>,
    #[arg(long = "private_key", help = "Private key used for signing blocks")]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub private_key: Option<String>,
}

impl std::fmt::Debug for SequencerOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SequencerOptions")
            .field("block_time", &self.block_time)
            .finish()
    }
}
