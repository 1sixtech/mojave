use clap::{Parser, Subcommand};
use std::fmt;
use tracing::Level;

#[derive(Parser)]
#[command(
    name = "mojave-prover",
    author,
    version,
    about = "Mojave Prover service for the Mojave network",
    arg_required_else_help = true
)]
pub struct Cli {
    #[arg(
        long = "log.level",
        value_name = "LOG_LEVEL",
        help = "The verbosity level used for logs.",
        long_help = "Possible values: info, debug, trace, warn, error",
        help_heading = "Prover options",
        global = true
    )]
    pub log_level: Option<Level>,
    #[arg(
        long = "datadir",
        value_name = "DATA_DIRECTORY",
        default_value = ".mojave/prover",
        help = "Directory for storing prover data.",
        long_help = "Specifies the directory where the prover will store its data.",
        help_heading = "Prover options",
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

#[derive(Parser)]
pub struct ProverOptions {
    #[arg(
        long = "prover.port",
        default_value = "3900",
        help = "Port for the prover",
        help_heading = "Prover Options"
    )]
    pub prover_port: u16,

    #[arg(
        long = "prover.host",
        default_value = "0.0.0.0",
        help = "Host for the prover",
        help_heading = "Prover Options"
    )]
    pub prover_host: String,

    #[arg(
        long = "prover.queue-capacity",
        default_value_t = 100,
        value_name = "CAPACITY",
        help = "Bounded mpsc queue capacity for proof jobs",
        help_heading = "Prover Options"
    )]
    pub queue_capacity: usize,

    #[arg(
        long = "prover.aligned-mode",
        help = "Enable aligned mode for proof generation",
        help_heading = "Prover Options"
    )]
    pub aligned_mode: bool,

    #[arg(
        long = "prover.private_key",
        help = "Private key used for signing proofs",
        help_heading = "Prover Options"
    )]
    pub private_key: String,

    #[arg(
        long = "no-daemon",
        help = "If set, the prover will run in the foreground (not as a daemon). By default, the prover runs as a daemon.",
        help_heading = "Daemon Options",
        action = clap::ArgAction::SetTrue
    )]
    pub no_daemon: bool,
}

impl fmt::Debug for ProverOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProverOpts")
            .field("prover_port", &self.prover_port)
            .field("prover_host", &self.prover_host)
            .field("queue_capacity", &self.queue_capacity)
            .field("aligned_mode", &self.aligned_mode)
            .field("private_key", &"[REDACTED]")
            .field("no_daemon", &self.no_daemon)
            .finish()
    }
}

#[derive(Subcommand)]

pub enum Command {
    #[command(name = "init", about = "Run the prover")]
    Start {
        #[command(flatten)]
        prover_options: ProverOptions,
    },

    #[command(name = "stop", about = "Stop the prover")]
    Stop,
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
        assert!(help.contains("mojave-prover"));
        assert!(help.to_lowercase().contains("prover options"));

        // version
        let version = Cli::command().render_version();
        assert!(!version.is_empty());
    }

    #[test]
    fn parse_start_with_defaults() {
        let cli = Cli::try_parse_from(["mojave-prover", "init", "--prover.private_key", "0xabc"])
            .unwrap();

        assert_eq!(cli.datadir, ".mojave/prover");
        assert!(cli.log_level.is_none());

        let Command::Start { ref prover_options } = cli.command else {
            panic!("expected start");
        };

        assert_eq!(prover_options.prover_port, 3900);
        assert_eq!(prover_options.prover_host, "0.0.0.0");
        assert_eq!(prover_options.queue_capacity, 100);
        assert!(!prover_options.aligned_mode);
        assert_eq!(prover_options.private_key, "0xabc");
        assert!(!prover_options.no_daemon);
    }

    #[test]
    fn parse_start_with_override() {
        let cli = Cli::try_parse_from([
            "mojave-prover",
            "init",
            "--log.level",
            "debug",
            "--datadir",
            "/tmp/prover",
            "--prover.port",
            "3901",
            "--prover.host",
            "127.0.0.1",
            "--prover.queue-capacity",
            "7",
            "--prover.aligned-mode",
            "--prover.private_key",
            "0xmojave",
            "--no-daemon",
        ])
        .unwrap();

        match cli.command {
            Command::Start { prover_options } => {
                assert_eq!(cli.log_level, Some(Level::DEBUG));
                assert_eq!(cli.datadir, "/tmp/prover");
                assert_eq!(prover_options.prover_port, 3901);
                assert_eq!(prover_options.prover_host, "127.0.0.1");
                assert_eq!(prover_options.queue_capacity, 7);
                assert!(prover_options.aligned_mode);
                assert_eq!(prover_options.private_key, "0xmojave");
                assert!(prover_options.no_daemon);
            }
            _ => panic!("expected start"),
        }
    }

    #[test]
    fn prover_options_debug_does_not_leak_private_key() {
        let opts = ProverOptions {
            prover_port: 3900,
            prover_host: "0.0.0.0".into(),
            queue_capacity: 7,
            aligned_mode: false,
            private_key: "0xabc".into(),
            no_daemon: true,
        };
        let dbg = format!("{opts:?}");

        assert!(dbg.contains("ProverOpts"));
        assert!(dbg.contains("[REDACTED]"));
        assert!(!dbg.contains("0xabc"));
    }

    #[test]
    fn parse_stop() {
        let cli = Cli::try_parse_from(["mojave-prover", "stop"]).unwrap();
        matches!(cli.command, Command::Stop);
    }

    #[test]
    fn parse_log_level() {
        let cli = Cli::try_parse_from([
            "mojave-prover",
            "--log.level",
            "debug",
            "init",
            "--prover.private_key",
            "0xabc",
        ])
        .unwrap();

        assert!(cli.log_level.is_some());
    }
}
