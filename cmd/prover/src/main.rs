pub mod cli;
use crate::cli::Command;

use anyhow::Result;
use mojave_daemon::{DaemonOptions, run_daemonized, stop_daemonized};
use mojave_prover_lib::start_api;

#[tokio::main]
async fn main() -> Result<()> {
    mojave_utils::logging::init();

    let cli = cli::Cli::run();

    if let Some(log_level) = cli.log_level {
        mojave_utils::logging::change_level(log_level);
    }

    match cli.command {
        Command::Start { prover_options } => {
            let bind_addr = format!(
                "{}:{}",
                prover_options.prover_host, prover_options.prover_port
            );

            let daemon_opts = DaemonOptions {
                no_daemon: prover_options.no_daemon,
                pid_file_path: prover_options.pid_file,
                log_file_path: prover_options.log_file,
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
            .await
            .unwrap_or_else(|err| tracing::error!("Failed to start daemonized node: {}", err));
        }
        Command::Stop { pid_file } => stop_daemonized(pid_file)?,
        _ => {} // Command::Status { rpc_url } => {}
                // Command::Proof(job_command) => match job_command {
                //     ProofCommand::Get { rpc_url, job_id } => {}
                //     ProofCommand::Pending { rpc_url } => {}
                // },
    }

    Ok(())
}
