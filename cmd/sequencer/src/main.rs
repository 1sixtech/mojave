pub mod cli;
mod k8s_leader;

use crate::{
    cli::Command,
    k8s_leader::{is_k8s_env, run_with_k8s_coordination, start_leader_tasks, stop_leader_tasks},
};
use anyhow::Result;

use mojave_block_producer::types::BlockProducerOptions;
use mojave_node_lib::{
    initializers::get_signer,
    types::{MojaveNode, NodeConfigFile},
    utils::store_node_config_file,
};
use mojave_proof_coordinator::types::ProofCoordinatorOptions;
use mojave_utils::{
    block_on::block_on_current_thread,
    daemon::{DaemonOptions, run_daemonized, stop_daemonized},
    p2p::public_key_from_signing_key,
};
use std::path::PathBuf;
use tracing::{error, info};

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
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;

            if let Err(e) =
                rt.block_on(async { MojaveNode::validate_node_options(&node_options).await })
            {
                error!("Failed to validate node options: {}", e);
                std::process::exit(1);
            }

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

                let use_k8s = is_k8s_env();

                if use_k8s {
                    run_with_k8s_coordination(
                        node.clone(),
                        node_options.clone(),
                        block_producer_options,
                        proof_coordinator_options,
                    )
                    .await?;
                } else {
                    let lt = start_leader_tasks(
                        node.clone(),
                        &node_options,
                        &block_producer_options,
                        &proof_coordinator_options,
                    )
                    .await?;

                    tokio::select! {
                        _ = mojave_utils::signal::wait_for_shutdown_signal() => {
                            info!("Termination signal received, shutting down sequencer..");
                            stop_leader_tasks(lt).await?;

                            let node_config_path = PathBuf::from(node.data_dir).join("node_config.json");
                            info!("Storing config at {:?}...", node_config_path);
                            let node_config = NodeConfigFile::new(node.peer_table.clone(), node.local_node_record.lock().await.clone()).await;
                            store_node_config_file(node_config, node_config_path).await;

                            // TODO: wait for api to stop here
                            // if let Err(_elapsed) = tokio::time::timeout(std::time::Duration::from_secs(10), api_task).await {
                            //     warn!("Timed out waiting for API to stop");
                            // }

                            info!("Successfully shut down the sequencer.");
                        }
                    }
                }
                Ok(())
            })
                .unwrap_or_else(|err| error!("Failed to start daemonized sequencer: {}", err));
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
