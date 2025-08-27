pub mod cli;

use crate::cli::Command;
use ethrex_p2p::network::public_key_from_signing_key;
use mojave_node_lib::{initializers::get_signer, types::MojaveNode};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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
        Command::GetPubKey { datadir } => {
            let signer = get_signer(&datadir.datadir)?;
            let public_key = public_key_from_signing_key(&signer);
            let public_key = hex::encode(public_key);
            println!("{public_key}");
        }
    }
    Ok(())
}
