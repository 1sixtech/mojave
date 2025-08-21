pub mod cli;

use crate::cli::{Cli, Command};
use mojave_block_producer::{BlockProducer, BlockProducerContext};
use mojave_client::MojaveClient;
use mojave_node_lib::Node;
use mojave_utils::logging::init_logging;
use reqwest::Url;
use std::{error::Error, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::run();
    init_logging(cli.log_level);
    match cli.command {
        Command::Init {
            options,
            sequencer_options,
        } => {
            let node = Node::init(&options).await.unwrap_or_else(|error| {
                tracing::error!("Failed to initialize the node: {}", error);
                std::process::exit(1);
            });

            let mojave_client = MojaveClient::new(sequencer_options.private_key.as_str())?;
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

            tokio::spawn(async move {
                loop {
                    match block_producer.build_block().await {
                        Ok(block) => mojave_client
                            .send_broadcast_block(&block, &full_node_urls)
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
                res = node.run(&options) => {
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

pub fn get_client_version() -> String {
    format!("{}/v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"),)
}
