use clap::{ArgAction, ArgGroup, Parser, Subcommand};
use mojave_block_producer::types::BlockProducerOptions;
use mojave_node_lib::types::{Node, SyncMode};
use mojave_proof_coordinator::types::ProofCoordinatorOptions;
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
        default_value = "full",
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
        long = "p2p.enabled",
        default_value = "true",
        value_name = "P2P_ENABLED",
        action = ArgAction::SetTrue,
        help_heading = "P2P options"
    )]
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
    #[arg(
        long = "no-daemon",
        help = "If set, the sequencer will run in the foreground (not as a daemon). By default, the sequencer runs as a daemon.",
        help_heading = "Daemon Options",
        action = clap::ArgAction::SetTrue
    )]
    pub no_daemon: bool,
}

impl From<&Options> for mojave_node_lib::types::NodeOptions {
    fn from(options: &Options) -> Self {
        Self {
            http_addr: None,
            http_port: None,
            authrpc_addr: None,
            authrpc_port: None,
            authrpc_jwtsecret: None,
            p2p_enabled: options.p2p_enabled,
            p2p_addr: options.p2p_addr.clone(),
            p2p_port: options.p2p_port.clone(),
            discovery_addr: options.discovery_addr.clone(),
            discovery_port: options.discovery_port.clone(),
            network: options.network.clone(),
            bootnodes: options.bootnodes.clone(),
            datadir: Default::default(),
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
    name = "mojave-sequencer",
    author,
    version,
    about = "Mojave is a blockchain node implementation for the Mojave network",
    arg_required_else_help = true
)]
pub struct Cli {
    #[arg(
        long = "log.level",
        value_name = "LOG_LEVEL",
        help = "The verbosity level used for logs.",
        long_help = "Possible values: info, debug, trace, warn, error",
        help_heading = "Node options",
        global = true
    )]
    pub log_level: Option<Level>,
    #[arg(
        long = "datadir",
        value_name = "DATABASE_DIRECTORY",
        help = "If the datadir is the word `memory`, ethrex will use the InMemory Engine",
        default_value = ".mojave/sequencer",
        help = "Receives the name of the directory where the Database is located.",
        long_help = "If the datadir is the word `memory`, ethrex will use the `InMemory Engine`.",
        help_heading = "Node options",
        env = "ETHREX_DATADIR",
        global = true
    )]
    pub datadir: String,
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn run() -> Self {
        Self::parse()
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
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

#[derive(Parser)]
#[clap(group(ArgGroup::new("mojave::SequencerOptions")))]
pub struct SequencerOptions {
    #[arg(
        long = "prover.address",
        help = "Allowed domain(s) and port(s) for the prover in the form 'domain:port'",
        help_heading = "Prover Options",
        default_value = "http://0.0.0.0:3900"
    )]
    pub prover_address: String,
    #[arg(
        long = "block_time",
        help = "Block creation interval in milliseconds",
        default_value = "1000"
    )]
    pub block_time: u64,
    #[arg(long = "private_key", help = "Private key used for signing blocks")]
    pub private_key: String,
}

impl std::fmt::Debug for SequencerOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SequencerOptions")
            .field("block_time", &self.block_time)
            .finish()
    }
}

impl From<&SequencerOptions> for BlockProducerOptions {
    fn from(value: &SequencerOptions) -> Self {
        Self {
            block_time: value.block_time,
            private_key: value.private_key.clone(),
        }
    }
}

impl From<&SequencerOptions> for ProofCoordinatorOptions {
    fn from(value: &SequencerOptions) -> Self {
        Self {
            prover_address: value.prover_address.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    #[test]
    fn help_and_version_render() {
        // help
        let mut cmd = Cli::command();
        let mut buf = Vec::new();
        cmd.write_help(&mut buf).unwrap();
        let help = String::from_utf8(buf).unwrap();
        assert!(help.contains("mojave-sequencer"));

        // version
        let version = Cli::command().render_version();
        assert!(!version.is_empty());
    }

    #[test]
    fn parse_start_minimal_uses_defaults() {
        let cli =
            Cli::try_parse_from(["mojave-sequencer", "init", "--private_key", "0xabc"]).unwrap();

        assert_eq!(cli.datadir, ".mojave/sequencer");
        assert!(cli.log_level.is_none());

        let Command::Start {
            ref options,
            ref sequencer_options,
        } = cli.command
        else {
            panic!("expected Start")
        };

        // Node Options defaults
        //assert_eq!(options.http_addr, "0.0.0.0");
        //assert_eq!(options.http_port, "8545");
        //assert_eq!(options.authrpc_addr, "localhost");
        //assert_eq!(options.authrpc_port, "8551");
        //assert_eq!(options.authrpc_jwtsecret, "jwt.hex");
        assert!(options.p2p_enabled, "p2p.enabled default should be true");
        assert_eq!(options.p2p_addr, "0.0.0.0");
        assert_eq!(options.p2p_port, "30303");
        assert_eq!(options.discovery_addr, "0.0.0.0");
        assert_eq!(options.discovery_port, "30303");

        // SequencerOptions defaults
        assert_eq!(sequencer_options.prover_address, "http://0.0.0.0:3900");
        assert_eq!(sequencer_options.block_time, 1000);
        assert_eq!(sequencer_options.private_key, "0xabc");

        // Even if it is Option<SyncMode>, syncmode must be Some(Full) because of default_value="full"
        assert!(matches!(options.syncmode, Some(SyncMode::Full)));
    }

    #[test]
    fn parse_start_with_overrides() {
        let cli = Cli::try_parse_from([
            "mojave-sequencer",
            "init",
            "--log.level",
            "debug",
            "--datadir",
            "/tmp/sequencer",
            "--prover.address",
            "http://127.0.0.1:3909",
            "--block_time",
            "2500",
            "--private_key",
            "0xmojave",
            "--p2p.addr",
            "127.0.0.1",
            "--p2p.port",
            "30304",
            "--discovery.addr",
            "127.0.0.1",
            "--discovery.port",
            "30305",
            "--metrics.addr",
            "0.0.0.0",
            "--metrics.port",
            "9393",
            "--metrics",
            "--force",
            "--syncmode",
            "snap",
            "--no-daemon",
        ])
        .unwrap();

        match cli.command {
            Command::Start {
                options,
                sequencer_options,
            } => {
                assert_eq!(cli.log_level, Some(Level::DEBUG));
                assert_eq!(cli.datadir, "/tmp/sequencer");

                assert_eq!(sequencer_options.prover_address, "http://127.0.0.1:3909");
                assert_eq!(sequencer_options.block_time, 2500);
                assert_eq!(sequencer_options.private_key, "0xmojave");

                //assert_eq!(options.http_addr, "127.0.0.1");
                //assert_eq!(options.http_port, "9000");
                //assert_eq!(options.authrpc_addr, "127.0.0.1");
                //assert_eq!(options.authrpc_port, "9001");
                //assert_eq!(options.authrpc_jwtsecret, "custom.jwt");
                assert_eq!(options.p2p_addr, "127.0.0.1");
                assert_eq!(options.p2p_port, "30304");
                assert_eq!(options.discovery_addr, "127.0.0.1");
                assert_eq!(options.discovery_port, "30305");
                assert_eq!(options.metrics_addr, "0.0.0.0");
                assert_eq!(options.metrics_port, "9393");
                assert!(options.metrics_enabled);
                assert!(options.force);
                assert!(matches!(options.syncmode, Some(SyncMode::Snap)));
            }
            _ => panic!("expected Start"),
        }
    }

    // #[test]
    // fn env_overrides_are_applied() {
    //     unsafe {
    //         std::env::set_var("ETHREX_DATADIR", "/env/dir");
    //         std::env::set_var("ETHREX_HTTP_ADDR", "10.0.0.1");
    //         std::env::set_var("ETHREX_HTTP_PORT", "7777");
    //         std::env::set_var("ETHREX_METRICS_PORT", "9191");
    //         std::env::set_var("ETHREX_NETWORK", "mainnet");
    //     }

    //     let cli =
    //         Cli::try_parse_from(["mojave-sequencer", "init", "--private_key", "0xabc"]).unwrap();

    //     match cli.command {
    //         Command::Start { options, .. } => {
    //             assert_eq!(cli.datadir, "/env/dir");
    //             assert_eq!(options.http_addr, "10.0.0.1");
    //             assert_eq!(options.http_port, "7777");
    //             assert_eq!(options.metrics_port, "9191");
    //             assert!(matches!(options.network, Network::Mainnet));
    //         }
    //         _ => panic!("expected Start"),
    //     }

    //     // clean
    //     unsafe {
    //         std::env::remove_var("ETHREX_DATADIR");
    //         std::env::remove_var("ETHREX_HTTP_ADDR");
    //         std::env::remove_var("ETHREX_HTTP_PORT");
    //         std::env::remove_var("ETHREX_METRICS_PORT");
    //         std::env::remove_var("ETHREX_NETWORK");
    //     }
    // }

    #[test]
    fn conversions_to_runtime_options_work() {
        // Options -> NodeOptions
        let cli = Cli::try_parse_from([
            "mojave-sequencer",
            "init",
            "--private_key",
            "0xabc",
            "--p2p.addr",
            "127.0.0.1",
            "--p2p.port",
            "30306",
            "--discovery.addr",
            "127.0.0.1",
            "--discovery.port",
            "30307",
            "--metrics.addr",
            "0.0.0.0",
            "--metrics.port",
            "9091",
            "--metrics",
            "--syncmode",
            "full",
        ])
        .unwrap();

        let (node_opts, seq_opts) = match cli.command {
            Command::Start {
                options,
                sequencer_options,
            } => (
                mojave_node_lib::types::NodeOptions::from(&options),
                sequencer_options,
            ),
            _ => panic!("expected Start"),
        };

        //assert_eq!(node_opts.http_addr, "1.2.3.4");
        //assert_eq!(node_opts.http_port, "9999");
        //assert_eq!(node_opts.authrpc_addr, "8.8.8.8");
        //assert_eq!(node_opts.authrpc_port, "8552");
        //assert_eq!(node_opts.authrpc_jwtsecret, "jwt2.hex");
        assert_eq!(node_opts.p2p_addr, "127.0.0.1");
        assert_eq!(node_opts.p2p_port, "30306");
        assert_eq!(node_opts.discovery_addr, "127.0.0.1");
        assert_eq!(node_opts.discovery_port, "30307");
        assert_eq!(node_opts.metrics_addr, "0.0.0.0");
        assert_eq!(node_opts.metrics_port, "9091");
        assert!(node_opts.metrics_enabled);
        assert!(matches!(node_opts.syncmode, SyncMode::Full));

        // SequencerOptions -> BlockProducerOptions
        let bp: BlockProducerOptions = (&seq_opts).into();
        assert_eq!(bp.block_time, seq_opts.block_time);
        assert_eq!(bp.private_key, seq_opts.private_key);

        // SequencerOptions -> ProofCoordinatorOptions
        let pc: ProofCoordinatorOptions = (&seq_opts).into();
        assert_eq!(pc.prover_address, seq_opts.prover_address);
    }

    #[test]
    fn sequencer_options_debug_does_not_leak_private_key() {
        let opts = SequencerOptions {
            prover_address: "http://0.0.0.0:3900".into(),
            block_time: 1000,
            private_key: "0xsecret".into(),
        };
        let dbg = format!("{opts:?}");

        assert!(dbg.contains("SequencerOptions"));
        assert!(dbg.contains("block_time: 1000"));
        assert!(!dbg.contains("0xsecret"));
    }

    #[test]
    fn parse_stop_and_get_pub_key() {
        let cli = Cli::try_parse_from(["mojave-sequencer", "stop"]).unwrap();
        matches!(cli.command, Command::Stop);

        let cli = Cli::try_parse_from(["mojave-sequencer", "get-pub-key"]).unwrap();
        matches!(cli.command, Command::GetPubKey);
    }

    #[test]
    fn invalid_bootnodes_string_rejected() {
        let res = Cli::try_parse_from(["mojave-sequencer", "init", "--bootnodes", "not-enode-url"]);
        assert!(res.is_err());
    }

    #[test]
    fn parse_log_level() {
        let cli = Cli::try_parse_from([
            "mojave-sequencer",
            "--log.level",
            "debug",
            "init",
            "--private_key",
            "0xabc",
        ])
        .unwrap();

        assert!(cli.log_level.is_some());
    }
}
