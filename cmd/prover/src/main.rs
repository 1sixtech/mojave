pub mod cli;
use crate::cli::{Command, ProofCommand};

use anyhow::Result;
use mojave_client::MojaveClient;
use mojave_prover_lib::start_api;
use mojave_utils::{
    daemon::{DaemonOptions, run_daemonized, stop_daemonized},
    runtime::execute_command_with_runtime,
};
use serde_json::json;

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
        Command::Status { rpc_url } => {
            let client = MojaveClient::builder()
                .prover_urls(&[rpc_url.clone()])
                .build()?;

            let reachable =
                execute_command_with_runtime(|| async move { client.request().get_job_id().await })
                    .is_ok();

            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "rpc": {
                        "url": rpc_url,
                        "reachable": reachable
                    }
                }))?
            );
        }
        Command::Proof(job_command) => match job_command {
            ProofCommand::Get { rpc_url, job_id } => {
                let client = MojaveClient::builder()
                    .prover_urls(&[rpc_url.clone()])
                    .build()?;
                let job_id_obj: mojave_client::types::JobId =
                    serde_json::from_value(json!(job_id))?;

                let proof = execute_command_with_runtime(|| async move {
                    client.request().get_proof(job_id_obj).await
                })?;
                println!("{}", serde_json::to_string_pretty(&proof)?);
            }
            ProofCommand::Pending { rpc_url } => {
                let client = MojaveClient::builder()
                    .prover_urls(&[rpc_url.clone()])
                    .build()?;
                let jobs =
                    execute_command_with_runtime(
                        || async move { client.request().get_job_id().await },
                    )?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({ "pending": jobs }))?
                );
            }
        },
    }

    Ok(())
}
