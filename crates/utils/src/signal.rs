use tokio::signal::unix::{signal, SignalKind};

// Waits for a termination signal and returns when one is received.
pub async fn wait_for_shutdown_signal() -> std::io::Result<()> {
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    let ctrl_c = tokio::signal::ctrl_c();

    tokio::select! {
        _ = sigint.recv() => {
            tracing::info!("Received SIGINT");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM");
        }
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C");
        }
    }

    Ok(())
}


