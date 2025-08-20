pub mod cli;

use crate::cli::{Cli, Command};
use mojave_node_lib::{Node, error::Error};
use mojave_utils::logging::init_logging;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = Cli::run();
    init_logging(cli.log_level);
    match cli.command {
        Command::Init {
            options,
            full_node_options,
        } => {
            let mut node = Node::new().await;
            tokio::select! {
                _ = node.run(options) => {
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
