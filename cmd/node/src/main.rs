pub mod cli;

use crate::cli::Command;
use anyhow::Result;
use mojave_node_lib::types::MojaveNode;

#[tokio::main]
async fn main() -> Result<()> {
    mojave_utils::logging::init();
    let cli = cli::Cli::run();

    if let Some(log_level) = cli.log_level {
        mojave_utils::logging::change_level(log_level);
    }
    match cli.command {
        Command::Start { options } => {
            let node_options: mojave_node_lib::types::NodeOptions = (&options).into();
            let node = MojaveNode::init(&node_options)
                .await
                .unwrap_or_else(|error| {
                    tracing::error!("Failed to initialize the node: {}", error);
                    std::process::exit(1);
                });
            tokio::select! {
                res = node.run(&node_options) => {
                    if let Err(err) = res {
                        tracing::error!("Node stopped unexpectedly: {}", err);
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Shutting down the full node..");
                }
            }
        }
    }
    Ok(())
}
