pub mod cli;

use crate::cli::Command;
use anyhow::Result;
use mojave_node_lib::{initializers::get_signer, rpc::context::RpcApiContext, types::MojaveNode};
use mojave_rpc_server::RpcRegistry;
use mojave_utils::{
    block_on::block_on_current_thread,
    daemon::{DaemonOptions, run_daemonized, stop_daemonized},
    p2p::public_key_from_signing_key,
};
use std::path::PathBuf;
use tracing::error;

const PID_FILE_NAME: &str = "node.pid";
const LOG_FILE_NAME: &str = "node.log";

fn main() -> Result<()> {
    let cli::Cli {
        command,
        datadir,
        log_level,
    } = cli::Cli::run();

    mojave_utils::logging::init(log_level);
    match command {
        Command::Start { options } => {
            let mut node_options: mojave_node_lib::types::NodeOptions = (&options).into();
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;

            if let Err(e) =
                rt.block_on(async { MojaveNode::validate_node_options(&node_options).await })
            {
                error!("Failed to validate node options: {}", e);
                std::process::exit(1);
            }

            node_options.datadir = datadir.clone();
            let daemon_opts = DaemonOptions {
                no_daemon: options.no_daemon,
                pid_file_path: PathBuf::from(datadir.clone()).join(PID_FILE_NAME),
                log_file_path: PathBuf::from(datadir).join(LOG_FILE_NAME),
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
                    |req, ctx: RpcApiContext| {
                        Box::pin(ethrex_rpc::map_eth_requests(req, ctx.l1_context))
                    },
                );

                node.run(&node_options, registry)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            })
            .unwrap_or_else(|err| {
                error!(error = %err, "Failed to start daemonized node");
            });
        }
        Command::Stop => stop_daemonized(PathBuf::from(datadir.clone()).join(PID_FILE_NAME))?,
        Command::GetPubKey => {
            let signer = block_on_current_thread(|| async move {
                get_signer(&datadir).await.map_err(anyhow::Error::from)
            })?;
            let public_key = public_key_from_signing_key(&signer);
            let public_key = hex::encode(public_key);
            println!("{public_key}");
        }
    }
    Ok(())
}
