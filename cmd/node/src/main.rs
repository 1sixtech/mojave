pub mod cli;
pub mod config;

use crate::{cli::Command, config::load_config};
use anyhow::Result;
use mojave_node_lib::{initializers::get_signer, types::MojaveNode};
use mojave_utils::p2p::public_key_from_signing_key;

#[tokio::main]
async fn main() -> Result<()> {
    mojave_utils::logging::init();
    let cli = cli::Cli::run();

    if let Some(log_level) = cli.log_level {
        mojave_utils::logging::change_level(log_level);
    }
    match cli.command {
        Command::Start { options } => {
            let node_options: mojave_node_lib::types::NodeOptions = load_config(options)?;
            let node = MojaveNode::init(&node_options).await.map_err(|error| {
                tracing::error!("Failed to initialize the node: {}", error);
                std::process::exit(1);
            })?;
            if let Err(err) = node.run(&node_options).await {
                tracing::error!("Node stopped unexpectedly: {}", err);
            }
        }
        Command::GetPubKey { datadir } => {
            let signer = get_signer(&datadir)?;
            let public_key = public_key_from_signing_key(&signer);
            let public_key = hex::encode(public_key);
            println!("{public_key}");
        }
    }
    Ok(())
}
