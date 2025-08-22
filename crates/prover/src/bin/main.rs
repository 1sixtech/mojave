use mojave_chain_utils::logging::init_logging;
use mojave_prover::{Cli, Command, Error, start_api};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = Cli::run();
    init_logging(cli.log_level);
    match cli.command {
        Command::Init { prover_options } => {
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
