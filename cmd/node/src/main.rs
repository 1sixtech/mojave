pub mod cli;
use crate::cli::Command;

use anyhow::Result;
use mojave_node_lib::{initializers::get_signer, types::MojaveNode};
use mojave_utils::p2p::public_key_from_signing_key;
use mojave_daemon::{DaemonOptions, run_daemonized, stop_daemonized};

#[tokio::main]
async fn main() -> Result<()> {
    mojave_utils::logging::init();
    let cli = cli::Cli::run();

    if let Some(log_level) = cli.log_level {
        mojave_utils::logging::change_level(log_level);
    }
    match cli.command {
        Command::Start { options } => {
            let node_options: mojave_node_lib::types::NodeOptions = (&options).into();
            let daemon_opts = DaemonOptions {
                no_daemon: options.no_daemon,
                pid_file_path: options.pid_file,
                log_file_path: options.log_file,
            };
            run_daemonized(daemon_opts, || async move {
                let node = MojaveNode::init(&node_options)
                    .await
                    .unwrap_or_else(|error| {
                        tracing::error!("Failed to initialize the node: {}", error);
                        std::process::exit(1);
                    });
                node.run(&node_options).await
            })
            .await
            .unwrap_or_else(|err| {
                tracing::error!("Failed to start daemonized node: {}", err);
            });
        },
        Command::Stop { pid_file } => {
            stop_daemonized(pid_file)?
        }
        Command::GetPubKey { datadir } => {
            let signer = get_signer(&datadir)?;
            let public_key = public_key_from_signing_key(&signer);
            let public_key = hex::encode(public_key);
            println!("{public_key}");
        }
    }
    Ok(())
}
