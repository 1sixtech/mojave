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
      default_value_t = Level::INFO,
      value_name = "LOG_LEVEL",
      help = "The verbosity level used for logs.",
      long_help = "Possible values: info, debug, trace, warn, error",
      help_heading = "Prover options")]
    pub log_level: Level,
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn run() -> Self {
        Self::parse()
    }
}

#[derive(Parser)]
pub struct ProverOpts {
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
        long = "prover.aligned-mode",
        help = "Enable aligned mode for proof generation",
        help_heading = "Prover Options"
    )]
    pub aligned_mode: bool,
}

impl Default for ProverOpts {
    fn default() -> Self {
        Self {
            prover_port: 3900,
            prover_host: "0.0.0.0".to_string(),
            aligned_mode: false,
        }
    }
}

impl fmt::Debug for ProverOpts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProverOpts")
            .field("prover_port", &self.prover_port)
            .field("prover_host", &self.prover_host)
            .field("aligned_mode", &self.aligned_mode)
            .finish()
    }
}

#[derive(Subcommand)]
pub enum Command {
    #[command(name = "init", about = "Run the prover")]
    Init {
        #[command(flatten)]
        prover_options: ProverOpts,
    },
}
