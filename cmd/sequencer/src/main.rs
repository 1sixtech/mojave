pub mod cli;

use anyhow::Result;

use mojave_block_producer::types::BlockProducerOptions;
use mojave_coordination::sequencer::run_sequencer;
use mojave_node_lib::types::MojaveNode;
use mojave_proof_coordinator::types::ProofCoordinatorOptions;
use mojave_utils::daemon::{DaemonOptions, run_daemonized};
use std::path::PathBuf;
use tracing::error;

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

        run_sequencer(
            node,
            &node_options,
            &block_producer_options,
            &proof_coordinator_options,
        )
        .await?;
        Ok(())
    })
    .unwrap_or_else(|err| error!("Failed to start daemonized sequencer: {}", err));

    Ok(())
}
