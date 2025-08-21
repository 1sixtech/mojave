pub mod cli;

use crate::cli::{Cli, Command};
use mojave_node_lib::{Node, error::Error};
use mojave_utils::logging::init_logging;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = Cli::run();
    init_logging(cli.log_level);
    match cli.command {
        Command::Init { options } => {
            let node = Node::init(&options).await.unwrap_or_else(|error| {
                tracing::error!("Failed to initialize the node: {}", error);
                std::process::exit(1);
            });
            tokio::select! {
                _ = node.run(&options) => {
                    tracing::error!("Node stopped unexpectedly");
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Shutting down the full node..");
                }
            }
        }
    }
    Ok(())
}
