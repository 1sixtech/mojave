pub mod cli;

use anyhow::{Context, Result};
use mojave_node_lib::{rpc::context::RpcApiContext, types::MojaveNode};
use mojave_rpc_core::types::Namespace;
use mojave_rpc_server::RpcRegistry;
use mojave_utils::daemon::{DaemonOptions, run_daemonized};
use std::path::PathBuf;
use tracing::{error, info};

const PID_FILE_NAME: &str = "node.pid";
const LOG_FILE_NAME: &str = "node.log";

fn main() -> Result<()> {
    let cli::Cli { command, options } = cli::Cli::run();

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
    info!("Starting Mojave Node...");

    let daemon_opts = build_daemon_options(&options.datadir, options.no_daemon);
    run_daemonized(daemon_opts, || async move {
        let node = MojaveNode::init(&node_options)
            .await
            .context("initialize node")
            .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e))?;

        let registry = build_registry();

        node.run(&node_options, registry)
            .await
            .context("run node")
            .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e))
    })
    .unwrap_or_else(|err| {
        error!(error = %err, "Failed to start daemonized node");
    });

    Ok(())
}

fn build_runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(Into::into)
}

fn build_node_options(options: &cli::Options) -> mojave_node_lib::types::NodeOptions {
    options.into()
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

fn build_registry() -> RpcRegistry<RpcApiContext> {
    RpcRegistry::new().with_fallback(Namespace::Eth, |req, ctx: RpcApiContext| {
        Box::pin(ethrex_rpc::map_eth_requests(req, ctx.l1_context))
    })
}

fn log_startup_config(options: &cli::Options) {
    info!(
        datadir = %options.datadir,
        network = %options.network,
        http = %format!("{}:{}", options.http_addr, options.http_port),
        authrpc = %format!("{}:{}", options.authrpc_addr, options.authrpc_port),
        health = %format!("{}:{}", options.health_addr, options.health_port),
        metrics = %format!("{}:{}", options.metrics_addr, options.metrics_port),
        p2p = %format!("{}:{}", options.p2p_addr, options.p2p_port),
        discovery = %format!("{}:{}", options.discovery_addr, options.discovery_port),
        "Node startup configuration"
    );
}
