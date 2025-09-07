pub mod cli;

use crate::cli::Command;
use anyhow::Result;
use mojave_block_producer::types::BlockProducerOptions;
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
        Command::Start {
            options,
            sequencer_options,
        } => {
            let node_options: mojave_node_lib::types::NodeOptions = (&options).into();
            let node = MojaveNode::init(&node_options)
                .await
                .unwrap_or_else(|error| {
                    tracing::error!("Failed to initialize the node: {}", error);
                    std::process::exit(1);
                });
            let block_producer_options: BlockProducerOptions = (&sequencer_options).into();
            if let Err(err) =
                mojave_block_producer::run(node, &node_options, &block_producer_options).await
            {
                tracing::error!("Sequencer stopped unexpectedly: {}", err);
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
