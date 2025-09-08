pub mod cli;

use crate::cli::Command;
use anyhow::Result;
use mojave_node_lib::{initializers::get_signer, types::MojaveNode};
use mojave_utils::{
    daemon::{DaemonOptions, run_daemonized, stop_daemonized},
    p2p::public_key_from_signing_key,
};

const PID_FILE_NAME: &str = "node.pid";
const LOG_FILE_NAME: &str = "node.pid";

fn main() -> Result<()> {
    mojave_utils::logging::init();
    let cli = cli::Cli::run();

    if let Some(log_level) = cli.log_level {
        mojave_utils::logging::change_level(log_level);
    }
    match cli.command {
        Command::Start { options } => {
            let mut node_options: mojave_node_lib::types::NodeOptions = (&options).into();
            node_options.datadir = cli.datadir.clone();
            let daemon_opts = DaemonOptions {
                no_daemon: options.no_daemon,
                pid_file_path: format!("{}/{}", cli.datadir, PID_FILE_NAME),
                log_file_path: format!("{}/{}", cli.datadir, LOG_FILE_NAME),
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
            .unwrap_or_else(|err| {
                tracing::error!("Failed to start daemonized node: {}", err);
            });
        }
        Command::Stop => stop_daemonized(format!("{}/{}", cli.datadir, PID_FILE_NAME))?,
        Command::GetPubKey => {
            let signer = get_signer(&cli.datadir)?;
            let public_key = public_key_from_signing_key(&signer);
            let public_key = hex::encode(public_key);
            println!("{public_key}");
        }
    }
    Ok(())
}
