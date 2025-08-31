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
#[derive(Clone)]
pub struct Cli {
    #[arg(
        long = "log.level",
        value_name = "LOG_LEVEL",
        help = "The verbosity level used for logs.",
        long_help = "Possible values: info, debug, trace, warn, error",
        help_heading = "Prover options"
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

#[derive(Clone, Parser)]
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

    #[arg(
        long = "pid.file",
        default_value = "mojave/prover.pid",
        value_name = "PID_FILE",
        help = "Path to the file where the prover's process ID (PID) will be written.",
        help_heading = "Daemon Options"
    )]
    pub pid_file: std::path::PathBuf,

    #[arg(
        long = "log.file",
        default_value = "mojave/prover.log",
        value_name = "LOG_FILE",
        help = "Path to the file where logs will be written.",
        help_heading = "Daemon Options"
    )]
    pub log_file: std::path::PathBuf,
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

#[derive(Clone, Subcommand)]

pub enum ProofCommand {
    #[command(name = "get", about = "Get proof of specific job id")]
    Get {
        #[arg(
            long = "rpc.url",
            default_value = "http://127.0.0.1:3900",
            help = "RPC URL of prover"
        )]
        rpc_url: String,

        #[arg(long = "job.id", help = "Job id to query proof")]
        job_id: String,
    },

    #[command(name = "pending", about = "List pending jobs")]
    Pending {
        #[arg(
            long = "rpc.url",
            default_value = "http://127.0.0.1:3900",
            help = "RPC URL of prover"
        )]
        rpc_url: String,
    },
}

#[derive(Clone, Subcommand)]

pub enum Command {
    #[command(name = "init", about = "Run the prover")]
    Start {
        #[command(flatten)]
        prover_options: ProverOptions,
    },

    #[command(name = "stop", about = "Stop the prover")]
    Stop {
        #[arg(
            long = "pid.file",
            default_value = "mojave/prover.pid",
            value_name = "PID_FILE",
            help = "Path to the file where the prover's process ID (PID) has written. (Default: inside the data directory)",
        )]
        pid_file: std::path::PathBuf
    },

    #[command(name = "status", about = "Check prover status")]
    Status {
        #[arg(
            long = "rpc.url",
            default_value = "http://127.0.0.1:3900",
            help = "RPC URL of prover"
        )]
        rpc_url: String,
    },

    #[command(
        subcommand,
        name = "proof",
        about = "Proof command to interact with prover"
    )]
    Proof(ProofCommand),
}
