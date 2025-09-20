use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Parser, Serialize, Deserialize, Debug, Clone)]
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
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub log_level: Option<String>,
    #[arg(
        long = "datadir",
        value_name = "DATA_DIRECTORY",
        help = "Directory for storing prover data.",
        long_help = "Specifies the directory where the prover will store its data.",
        help_heading = "Prover options"
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

#[derive(Parser, Serialize, Deserialize, Clone)]
pub struct ProverOptions {
    #[arg(
        long = "prover.port",
        help = "Port for the prover",
        help_heading = "Prover Options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub prover_port: Option<u16>,

    #[arg(
        long = "prover.host",
        help = "Host for the prover",
        help_heading = "Prover Options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub prover_host: Option<String>,

    #[arg(
        long = "prover.queue-capacity",
        value_name = "CAPACITY",
        help = "Bounded mpsc queue capacity for proof jobs",
        help_heading = "Prover Options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub queue_capacity: Option<usize>,

    #[arg(
        long = "prover.aligned-mode",
        help = "Enable aligned mode for proof generation",
        help_heading = "Prover Options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub aligned_mode: Option<bool>,

    #[arg(
        long = "prover.private_key",
        help = "Private key used for signing proofs",
        help_heading = "Prover Options"
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub private_key: Option<String>,

    #[arg(
        long = "no-daemon",
        help = "If set, the prover will run in the foreground (not as a daemon). By default, the prover runs as a daemon.",
        help_heading = "Daemon Options",
        action = clap::ArgAction::SetTrue
    )]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub no_daemon: Option<bool>,
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

#[derive(Subcommand, Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Command {
    #[command(name = "init", about = "Run the prover")]
    Start {
        #[command(flatten)]
        #[serde(flatten)]
        prover_options: ProverOptions,
    },

    #[command(name = "stop", about = "Stop the prover")]
    Stop,
}
