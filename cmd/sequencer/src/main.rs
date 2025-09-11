pub mod cli;
pub mod config;

use crate::{cli::Command, config::load_config};
use anyhow::Result;

use mojave_block_producer::types::BlockProducerOptions;
use mojave_node_lib::{initializers::get_signer, types::MojaveNode};
use mojave_utils::{
    daemon::{DaemonOptions, run_daemonized, stop_daemonized},
    p2p::public_key_from_signing_key,
};
use tracing::Level;
use std::path::PathBuf;
use std::str::FromStr;

const PID_FILE_NAME: &str = "sequencer.pid";
const LOG_FILE_NAME: &str = "sequencer.log";

fn main() -> Result<()> {
    mojave_utils::logging::init();
    let cli = cli::Cli::run();
    let config = load_config(&cli)?;

    if let Some(log_level) = &cli.log_level {
        mojave_utils::logging::change_level(Level::from_str(log_level)?);
    }
    match cli.command {
        Command::Start {
            options: _,
            sequencer_options: _,
        } => {
            let node_options: mojave_node_lib::types::NodeOptions = (&config).into();
            
            let block_producer_options: BlockProducerOptions = (&config).into();
            let daemon_opts = DaemonOptions {
                no_daemon: config.no_daemon,
                pid_file_path: PathBuf::from(config.datadir.clone()).join(PID_FILE_NAME),
                log_file_path: PathBuf::from(config.datadir).join(LOG_FILE_NAME),
            };

            run_daemonized(daemon_opts, || async move {
                let node = MojaveNode::init(&node_options)
                    .await
                    .unwrap_or_else(|error| {
                        tracing::error!("Failed to initialize the node: {}", error);
                        std::process::exit(1);
                    });
                mojave_block_producer::run(node, &node_options, &block_producer_options)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            })
            .unwrap_or_else(|err| {
                tracing::error!("Failed to start daemonized node: {}", err);
            });
        }
        Command::Stop => stop_daemonized(PathBuf::from(config.datadir.clone()).join(PID_FILE_NAME))?,
        Command::GetPubKey => {
            let signer = get_signer(&config.datadir)?;
            let public_key = public_key_from_signing_key(&signer);
            let public_key = hex::encode(public_key);
            println!("{public_key}");
        }
    }
    Ok(())
}
