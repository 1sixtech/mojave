use mojave_chain_utils::logging::init_logging;
use mojave_prover::{Cli, Command, ProverServer};

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
