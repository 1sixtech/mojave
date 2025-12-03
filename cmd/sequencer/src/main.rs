pub mod cli;

use anyhow::{Context, Result};

use mojave_block_producer::types::BlockProducerOptions;
use mojave_coordination::sequencer::run_sequencer;
use mojave_node_lib::types::MojaveNode;
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

    let rt = build_runtime()?;

    if let Some(subcommand) = command {
        return rt.block_on(async { subcommand.run(options.datadir.clone()).await });
    }

    let node_options = build_node_options(&options);
    if let Err(e) = validate_node_options(&rt, &node_options) {
        error!("Failed to validate node options: {}", e);
        std::process::exit(1);
    }

    log_startup_config(&options);
    info!("Starting Sequencer...");

    let block_producer_options: BlockProducerOptions = (&sequencer_options).into();
    let proof_coordinator_options: ProofCoordinatorOptions = (&sequencer_options).into();
    let daemon_opts = build_daemon_options(&options.datadir, options.no_daemon);

    run_daemonized(daemon_opts, || async move {
        let node = MojaveNode::init(&node_options)
            .await
            .context("initialize sequencer node")
            .map_err(Box::<dyn std::error::Error + Send + Sync>::from)?;

        run_sequencer(
            node,
            &node_options,
            &block_producer_options,
            &proof_coordinator_options,
        )
        .await
        .map_err(|e| {
            error!("Sequencer run failed: {e:?}");
            e
        })
    })
    .unwrap_or_else(|err| error!("Failed to start daemonized sequencer: {}", err));

    Ok(())
}

fn build_runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(Into::into)
}

fn build_node_options(options: &cli::Options) -> mojave_node_lib::types::NodeOptions {
    let node_options: mojave_node_lib::types::NodeOptions = options.into();
    node_options
}

fn validate_node_options(
    rt: &tokio::runtime::Runtime,
    node_options: &mojave_node_lib::types::NodeOptions,
) -> Result<()> {
    rt.block_on(async { MojaveNode::validate_node_options(node_options).await })
        .context("validate node options")
}

fn build_daemon_options(datadir: &str, no_daemon: bool) -> DaemonOptions {
    DaemonOptions {
        no_daemon,
        pid_file_path: PathBuf::from(datadir).join(PID_FILE_NAME),
        log_file_path: PathBuf::from(datadir).join(LOG_FILE_NAME),
    }
}

fn log_startup_config(options: &cli::Options) {
    info!(
        datadir = %options.datadir,
        network = %options.network,
        health = %format!("{}:{}", options.health_addr, options.health_port),
        metrics = %format!("{}:{}", options.metrics_addr, options.metrics_port),
        p2p_enabled = options.p2p_enabled,
        p2p = %format!("{}:{}", options.p2p_addr, options.p2p_port),
        discovery = %format!("{}:{}", options.discovery_addr, options.discovery_port),
        "Sequencer startup configuration"
    );
}
