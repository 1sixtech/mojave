pub mod cli;

use anyhow::Result;
use mojave_node_lib::{rpc::context::RpcApiContext, types::MojaveNode};
use mojave_rpc_server::RpcRegistry;
use mojave_utils::daemon::{DaemonOptions, run_daemonized};
use std::path::PathBuf;
use tracing::error;

const PID_FILE_NAME: &str = "node.pid";
const LOG_FILE_NAME: &str = "node.log";

fn main() -> Result<()> {
    let cli::Cli { command, options } = cli::Cli::run();

    mojave_utils::logging::init(options.log_level);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let mut node_options: mojave_node_lib::types::NodeOptions = (&options).into();

    if let Some(subcommand) = command {
        return rt.block_on(async { subcommand.run(options.datadir.clone()).await });
    }

    if let Err(e) = rt.block_on(async { MojaveNode::validate_node_options(&node_options).await }) {
        error!("Failed to validate node options: {}", e);
        std::process::exit(1);
    }

    println!("Starting Mojave Node...");

    node_options.datadir = options.datadir.clone();
    let daemon_opts = DaemonOptions {
        no_daemon: options.no_daemon,
        pid_file_path: PathBuf::from(options.datadir.clone()).join(PID_FILE_NAME),
        log_file_path: PathBuf::from(options.datadir).join(LOG_FILE_NAME),
    };
    run_daemonized(daemon_opts, || async move {
        let node = MojaveNode::init(&node_options)
            .await
            .unwrap_or_else(|error| {
                error!("Failed to initialize the node: {}", error);
                std::process::exit(1);
            });

        let registry = RpcRegistry::new().with_fallback(
            mojave_rpc_core::types::Namespace::Eth,
            |req, ctx: RpcApiContext| Box::pin(ethrex_rpc::map_eth_requests(req, ctx.l1_context)),
        );

        node.run(&node_options, registry)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    })
    .unwrap_or_else(|err| {
        error!(error = %err, "Failed to start daemonized node");
    });

    Ok(())
}
