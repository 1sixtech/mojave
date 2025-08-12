use clap::{Parser, Subcommand};
use tracing::Level;

use mojave_chain_utils::{logging::init_logging, prover_options::ProverOpts};

use mojave_prover::ProverServer;

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
      help_heading = "Node options")]
    pub log_level: Level,
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
    #[command(name = "init", about = "Run the prover")]
    Init {
        #[command(flatten)]
        prover_options: ProverOpts,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::run();
    init_logging(cli.log_level);
    match cli.command {
        Command::Init { prover_options } => {
            tracing::info!(
                "Prover starting on {}:{} (aligned_mode: {})",
                prover_options.prover_host,
                prover_options.prover_port,
                prover_options.aligned_mode
            );

            let bind_addr = format!(
                "{}:{}",
                prover_options.prover_host, prover_options.prover_port
            );
            let mut server = ProverServer::new(prover_options.aligned_mode, &bind_addr).await;

            tokio::select! {
                _ = server.start() => {
                    tracing::error!("Prover stopped unexpectedly");
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Shutting down prover...");
                }
            }
        }
    }
}
