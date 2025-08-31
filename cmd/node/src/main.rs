pub mod cli;

use crate::cli::Command;
use mojave_daemon::{DaemonOptions, run_daemonized, stop_daemonized};
use mojave_node_lib::types::MojaveNode;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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
        }
        Command::Stop { pid_file } => stop_daemonized(pid_file)?,
    }
    Ok(())
}
