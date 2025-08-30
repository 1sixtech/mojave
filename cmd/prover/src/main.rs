pub mod cli;

use crate::cli::Command;

use anyhow::Result;
use mojave_prover_lib::start_api;

#[tokio::main]
async fn main() -> Result<()> {
    mojave_utils::logging::init();
    let cli = cli::Cli::run();

    match cli.command {
        Command::Start { prover_options } => {
            tracing::info!(
                prover_host = %prover_options.prover_host,
                prover_port = %prover_options.prover_port,
                aligned_mode = %prover_options.aligned_mode,
                queue_capacity = %prover_options.queue_capacity,
                "Prover starting with configuration"
            );

            let bind_addr = format!(
                "{}:{}",
                prover_options.prover_host, prover_options.prover_port
            );

            tokio::select! {
                res = start_api(prover_options.aligned_mode,  &bind_addr, &prover_options.private_key, prover_options.queue_capacity) => {
                    match res {
                        Ok(()) => tracing::error!("Prover stopped unexpectedly"),
                        Err(err) => tracing::error!("Prover stopped with error: {:}", err),
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Shutting down prover...");
                }
            }
        }
    }
    Ok(())
}
