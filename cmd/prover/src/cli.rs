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
        help_heading = "Prover options"
    )]
    pub log_level: Option<Level>,
    #[arg(
        long = "datadir",
        value_name = "DATA_DIRECTORY",
        default_value = ".mojave/prover",
        help = "Directory for storing prover data.",
        long_help = "Specifies the directory where the prover will store its data.",
        help_heading = "Prover options",
        env = "ETHREX_DATADIR"
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
