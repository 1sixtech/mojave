pub mod cli;

use crate::cli::Command;
use anyhow::Result;
use std::sync::Arc;

use mojave_batch_producer::{BatchProducer, types::Request as BatchProducerRequest};
use mojave_batch_submitter::queue::NoOpBatchQueue;
use mojave_block_producer::{
    BlockProducer,
    types::{BlockProducerOptions, Request as BlockProducerRequest},
};
use mojave_node_lib::{
    initializers::get_signer,
    types::{MojaveNode, NodeConfigFile},
    utils::store_node_config_file,
};
use mojave_proof_coordinator::{ProofCoordinator, types::ProofCoordinatorOptions};
use mojave_task::Task;
use mojave_utils::{
    block_on::block_on_current_thread,
    daemon::{DaemonOptions, run_daemonized, stop_daemonized},
    p2p::public_key_from_signing_key,
};
use std::{path::PathBuf, time::Duration};

const PID_FILE_NAME: &str = "sequencer.pid";
const LOG_FILE_NAME: &str = "sequencer.log";
const BLOCK_PRODUCER_CAPACITY: usize = 100;

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

                let batch_queue = Arc::new(NoOpBatchQueue::new());
                let batch_producer = BatchProducer::new(node.clone(), 0, batch_queue);
                let block_producer = BlockProducer::new(node.clone());
                let proof_coordinator = ProofCoordinator::new(node.clone(), &node_options, &proof_coordinator_options)?;

                let batch_producer_task = batch_producer
                    .spawn_periodic(Duration::from_millis(10_000), || BatchProducerRequest::BuildBatch);

                let block_producer_task = block_producer
                    .spawn_with_capacity_periodic(BLOCK_PRODUCER_CAPACITY, Duration::from_millis(block_producer_options.block_time), || BlockProducerRequest::BuildBlock);

                // TODO: add batch submitter handle here

                let proof_coordinator_task = proof_coordinator.spawn();

                tokio::select! {
                    // TODO: replace with api task

                    _ = mojave_utils::signal::wait_for_shutdown_signal()  => {
                        tracing::info!("Termination signal received, shutting down sequencer..");
                        cancel_token.cancel();
                        batch_producer_task.shutdown().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                        block_producer_task.shutdown().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                        proof_coordinator_task.shutdown().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

                        let node_config_path = PathBuf::from(node.data_dir).join("node_config.json");
                        tracing::info!("Storing config at {:?}...", node_config_path);

                        let node_config = NodeConfigFile::new(node.peer_table.clone(), node.local_node_record.lock().await.clone()).await;
                        store_node_config_file(node_config, node_config_path).await;

                        // TODO: wait for api to stop here
                        // if let Err(_elapsed) = tokio::time::timeout(std::time::Duration::from_secs(10), api_task).await {
                        //     tracing::warn!("Timed out waiting for API to stop");
                        // }

                        tracing::info!("Successfully shut down the sequencer.");
                        Ok(())
                    }
                }
            })
            .unwrap_or_else(|err| tracing::error!("Failed to start daemonized sequencer: {}", err));
        }
        Command::Stop => stop_daemonized(PathBuf::from(cli.datadir.clone()).join(PID_FILE_NAME))?,
        Command::GetPubKey => {
            let signer = block_on_current_thread(|| async move {
                get_signer(&cli.datadir).await.map_err(anyhow::Error::from)
            })?;
            let public_key = public_key_from_signing_key(&signer);
            let public_key = hex::encode(public_key);
            println!("{public_key}");
        }
    }
    Ok(())
}
