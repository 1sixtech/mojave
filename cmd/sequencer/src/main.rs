pub mod cli;

use crate::cli::Command;
use anyhow::Result;

use mojave_block_producer::types::BlockProducerOptions;
use mojave_node_lib::{initializers::get_signer, types::MojaveNode};
use mojave_utils::{
    daemon::{DaemonOptions, run_daemonized, stop_daemonized},
    p2p::public_key_from_signing_key,
};
use std::path::PathBuf;

const PID_FILE_NAME: &str = "sequencer.pid";
const LOG_FILE_NAME: &str = "sequencer.log";

fn main() -> Result<()> {
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
            let mut node_options: mojave_node_lib::types::NodeOptions = (&options).into();
            node_options.datadir = cli.datadir.clone();
            let block_producer_options: BlockProducerOptions = (&sequencer_options).into();
            let daemon_opts = DaemonOptions {
                no_daemon: options.no_daemon,
                pid_file_path: PathBuf::from(cli.datadir.clone()).join(PID_FILE_NAME),
                log_file_path: PathBuf::from(cli.datadir).join(LOG_FILE_NAME),
            };

            run_daemonized(daemon_opts, || async move {
                let node = MojaveNode::init(&node_options)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                mojave_block_producer::run(node, &node_options, &block_producer_options)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            })?;
        }
        Command::Stop => stop_daemonized(PathBuf::from(cli.datadir.clone()).join(PID_FILE_NAME))?,
        Command::GetPubKey => {
            let signer = get_signer(&cli.datadir)?;
            let public_key = public_key_from_signing_key(&signer);
            let public_key = hex::encode(public_key);
            println!("{public_key}");
        }
    }
    Ok(())
}
