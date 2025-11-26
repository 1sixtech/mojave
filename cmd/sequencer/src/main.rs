pub mod cli;
mod k8s_leader;

use crate::k8s_leader::{
    is_k8s_env, run_with_k8s_coordination, start_leader_tasks, stop_leader_tasks,
};
use anyhow::Result;

use mojave_block_producer::types::BlockProducerOptions;
use mojave_node_lib::{
    types::{MojaveNode, NodeConfigFile},
    utils::store_node_config_file,
};
use mojave_proof_coordinator::types::ProofCoordinatorOptions;
use mojave_utils::daemon::{DaemonOptions, run_daemonized};
use std::path::PathBuf;
use tracing::{error, info};

const PID_FILE_NAME: &str = "sequencer.pid";
const LOG_FILE_NAME: &str = "sequencer.log";

fn main() -> Result<()> {
    let cli::Cli {
        command,
        options,
        sequencer_options,
    } = cli::Cli::run();

    mojave_utils::logging::init(options.log_level);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    if let Some(subcommand) = command {
        return rt.block_on(async { subcommand.run(options.datadir.clone()).await });
    }

    let node_options: mojave_node_lib::types::NodeOptions = (&options).into();

    if let Err(e) = rt.block_on(async { MojaveNode::validate_node_options(&node_options).await }) {
        error!("Failed to validate node options: {}", e);
        std::process::exit(1);
    }

    let block_producer_options: BlockProducerOptions = (&sequencer_options).into();
    let proof_coordinator_options: ProofCoordinatorOptions = (&sequencer_options).into();
    let daemon_opts = DaemonOptions {
        no_daemon: options.no_daemon,
        pid_file_path: PathBuf::from(options.datadir.clone()).join(PID_FILE_NAME),
        log_file_path: PathBuf::from(options.datadir).join(LOG_FILE_NAME),
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
    Ok(())
}
