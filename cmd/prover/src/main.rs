pub mod cli;

use crate::cli::Command;
use anyhow::Result;
use clap::Parser;
use mojave_prover_lib::start_api;
use mojave_utils::daemon::{DaemonOptions, run_daemonized, stop_daemonized};
use std::path::PathBuf;

const PID_FILE_NAME: &str = "prover.pid";
const LOG_FILE_NAME: &str = "prover.log";

fn main() -> Result<()> {
    let cli::Cli {
        log_level,
        datadir,
        command,
    } = cli::Cli::parse();

    mojave_utils::logging::init(log_level);

    match command {
        Command::Start { prover_options } => {
            let bind_addr = format!(
                "{}:{}",
                prover_options.prover_host, prover_options.prover_port
            );

            let daemon_opts = DaemonOptions {
                no_daemon: prover_options.no_daemon,
                pid_file_path: PathBuf::from(datadir.clone()).join(PID_FILE_NAME),
                log_file_path: PathBuf::from(datadir).join(LOG_FILE_NAME),
            };

            run_daemonized(daemon_opts, || async move {
                start_api(
                    prover_options.aligned_mode,
                    &bind_addr,
                    &prover_options.private_key,
                    prover_options.queue_capacity,
                )
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            })
            .unwrap_or_else(|err| tracing::error!("Failed to start daemonized prover: {}", err));
        }
        Command::Stop => stop_daemonized(PathBuf::from(datadir.clone()).join(PID_FILE_NAME))?,
    }

    Ok(())
}
