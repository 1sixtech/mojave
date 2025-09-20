use tokio::signal::unix::{SignalKind, signal};

pub async fn wait_for_shutdown_signal() -> std::io::Result<()> {
    let mut sigterm = signal(SignalKind::terminate())?;
    let ctrl_c = tokio::signal::ctrl_c();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C (SIGINT)");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM");
        }
    }

    Ok(())
}
