use std::{future::Future, net::SocketAddr};

use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tracing::info;

/// Background task handle for the health probe server.
pub type HealthProbeHandle = JoinHandle<std::io::Result<()>>;

/// Spawn a lightweight HTTP server exposing a `/health` endpoint.
///
/// The server binds the provided socket address (use port `0` to pick an ephemeral
/// port) and serves `GET /health` with a 200 OK response. The returned handle can
/// be awaited to surface server errors; the server stops when `shutdown_signal`
/// resolves.
pub async fn spawn_health_probe<F>(
    addr: SocketAddr,
    shutdown_signal: F,
) -> Result<(SocketAddr, HealthProbeHandle), std::io::Error>
where
    F: Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind(addr).await?;
    let bound_addr = listener.local_addr()?;

    info!("Health probe listening on {bound_addr}");

    let handle = tokio::spawn(async move {
        tokio::pin!(shutdown_signal);

        loop {
            tokio::select! {
                _ = &mut shutdown_signal => break,
                accept_res = listener.accept() => {
                    let (mut stream, _) = accept_res?;
                    respond_ok(&mut stream).await?;
                }
            }
        }

        Ok(())
    });

    Ok((bound_addr, handle))
}

async fn respond_ok(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = [0u8; 1024];
    let _ = stream.readable().await;
    let _ = stream.try_read(&mut buf);

    const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\ncontent-type: text/plain\r\nconnection: close\r\n\r\nOK";
    stream.write_all(RESPONSE).await?;
    stream.shutdown().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::AsyncReadExt, sync::oneshot};

    #[tokio::test]
    async fn health_probe_serves_ok() {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (addr, handle) = spawn_health_probe("127.0.0.1:0".parse().unwrap(), async {
            let _ = shutdown_rx.await;
        })
        .await
        .expect("start health probe");

        let mut stream = tokio::net::TcpStream::connect(addr)
            .await
            .expect("connect to health probe");
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .expect("write request");

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.expect("read response");

        let resp = String::from_utf8_lossy(&buf);
        assert!(
            resp.starts_with("HTTP/1.1 200 OK"),
            "unexpected response: {resp}"
        );
        assert!(resp.contains("\r\n\r\nOK"), "missing body: {resp}");

        // Trigger graceful shutdown and surface any server errors
        let _ = shutdown_tx.send(());
        handle.await.unwrap().unwrap();
    }
}
