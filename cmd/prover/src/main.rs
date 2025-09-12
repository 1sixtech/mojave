pub mod cli;
pub mod config;

use crate::cli::Command;
use anyhow::Result;
use mojave_prover_lib::start_api;
use mojave_utils::daemon::{DaemonOptions, run_daemonized, stop_daemonized};

use std::{path::PathBuf, str::FromStr};
use tracing::Level;

const PID_FILE_NAME: &str = "prover.pid";
const LOG_FILE_NAME: &str = "prover.log";

fn main() -> Result<()> {
    mojave_utils::logging::init();

    let cli = cli::Cli::run();
    let config = config::load_config(&cli)?;

    if let Some(log_level) = &cli.log_level {
        mojave_utils::logging::change_level(Level::from_str(log_level)?);
    }

    match cli.command {
        Command::Start { prover_options: _ } => {
            let bind_addr = format!("{}:{}", config.prover_host, config.prover_port);

            let daemon_opts = DaemonOptions {
                no_daemon: config.no_daemon,
                pid_file_path: PathBuf::from(config.datadir.clone()).join(PID_FILE_NAME),
                log_file_path: PathBuf::from(config.datadir).join(LOG_FILE_NAME),
            };

            run_daemonized(daemon_opts, || async move {
                start_api(
                    config.aligned_mode,
                    &bind_addr,
                    &config.private_key,
                    config.queue_capacity,
                )
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            })
            .unwrap_or_else(|err| tracing::error!("Failed to start daemonized node: {}", err));
        }
        Command::Stop => {
            stop_daemonized(PathBuf::from(config.datadir.clone()).join(PID_FILE_NAME))?
        }
    }

    Ok(())
}
