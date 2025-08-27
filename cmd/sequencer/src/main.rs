pub mod cli;

use crate::cli::Command;
use mojave_block_producer::{BlockProducer, BlockProducerContext};
use mojave_client::MojaveClient;
use mojave_node_lib::types::MojaveNode;
use reqwest::Url;
use std::{error::Error, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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

            let context = BlockProducerContext::new(
                node.store.clone(),
                node.blockchain.clone(),
                node.rollup_store.clone(),
                node.genesis.coinbase,
            );
            let block_producer = BlockProducer::start(context, 100);
            let full_node_urls: Vec<Url> = sequencer_options
                .full_node_addresses
                .iter()
                .map(|address| {
                    Url::parse(address)
                        .unwrap_or_else(|error| panic!("Failed to parse URL: {error}"))
                })
                .collect();

            let mojave_client = MojaveClient::builder()
                .private_key(&sequencer_options.private_key)
                .unwrap_or_else(|error| {
                    tracing::error!("Failed to parse private key: {}", error);
                    std::process::exit(1);
                })
                .build()
                .unwrap_or_else(|error| {
                    tracing::error!("Failed to build the client: {}", error);
                    std::process::exit(1);
                });

            tokio::spawn(async move {
                loop {
                    match block_producer.build_block().await {
                        Ok(block) => mojave_client
                            .request_builder()
                            .full_node_urls(&full_node_urls)
                            .send_broadcast_block(&block)
                            .await
                            .unwrap_or_else(|error| tracing::error!("{}", error)),
                        Err(error) => {
                            tracing::error!("Failed to build a block: {}", error);
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(sequencer_options.block_time)).await;
                }
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
