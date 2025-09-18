pub mod cli;

use crate::cli::Command;
use anyhow::Result;

use mojave_batch_submitter::{committer::Committer, notifier::Notifier};
use mojave_block_producer::types::BlockProducerOptions;
use mojave_node_lib::{initializers::get_signer, types::MojaveNode};
use mojave_proof_coordinator::types::ProofCoordinatorOptions;
use mojave_utils::{
    block_on::run_on_tokio_single,
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
            let proof_coordinator_options: ProofCoordinatorOptions = (&sequencer_options).into();
            let daemon_opts = DaemonOptions {
                no_daemon: options.no_daemon,
                pid_file_path: PathBuf::from(cli.datadir.clone()).join(PID_FILE_NAME),
                log_file_path: PathBuf::from(cli.datadir).join(LOG_FILE_NAME),
            };

            run_daemonized(daemon_opts, || async move {
                let node = MojaveNode::init(&node_options)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

                let cancel_token = node.cancel_token.clone();

                let mut block_producer_task = Box::pin(mojave_block_producer::run(
                    node.clone(),
                    &node_options,
                    &block_producer_options,
                ));

                let (batch_tx, batch_rx) = tokio::sync::mpsc::channel(16);

                let coordinator_task = Box::pin(mojave_proof_coordinator::run(
                    node,
                    &node_options,
                    &proof_coordinator_options,
                    batch_rx,
                ));

                let batch_submitter_task = Box::pin(Committer::<Notifier>::run(batch_tx));

                tokio::select! {
                    res = &mut block_producer_task => {
                        res.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                    }

                    _ = mojave_utils::signal::wait_for_shutdown_signal()  => {
                        tracing::info!("Termination signal received, shutting down sequencer..");
                        cancel_token.cancel();
                        block_producer_task
                            .await
                            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                        coordinator_task
                            .await
                            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                        batch_submitter_task
                            .await
                            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                    }
                }
            })
            .unwrap_or_else(|err| tracing::error!("Failed to start daemonized sequencer: {}", err));
        }
        Command::Stop => stop_daemonized(PathBuf::from(cli.datadir.clone()).join(PID_FILE_NAME))?,
        Command::GetPubKey => {
            let signer = run_on_tokio_single(|| async move {
                get_signer(&cli.datadir).await.map_err(anyhow::Error::from)
            })?;
            let public_key = public_key_from_signing_key(&signer);
            let public_key = hex::encode(public_key);
            println!("{public_key}");
        }
    }
    Ok(())
}
