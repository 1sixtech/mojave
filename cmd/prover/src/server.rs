use crate::message::{self, MessageError, Request, Response};
use ethrex_prover_lib::{backends::Backend, prove, to_batch_proof};
use tokio::net::{TcpListener, TcpStream};

#[allow(unused)]
const QUEUE_SIZE: usize = 100;

#[allow(unused)]
pub struct ProverServer {
    aligned_mode: bool,
    tcp_listener: TcpListener,
}

impl ProverServer {
    /// Creates a new instance of the Prover.
    ///
    /// ```rust,ignore
    /// use mojave_prover::ProverServer;
    ///
    /// let (mut prover, _, _) = ProverServer::new(true);
    /// tokio::spawn(async move {
    ///     prover.start().await;
    /// });
    /// ```
    #[allow(unused)]
    pub async fn new(aligned_mode: bool, bind_addr: &str) -> Self {
        let tcp_listener = TcpListener::bind(bind_addr)
            .await
            .expect("TcpListener bind error");
        ProverServer {
            aligned_mode,
            tcp_listener,
        }
    }

    #[allow(unused)]
    pub async fn start(&mut self) {
        loop {
            match self.tcp_listener.accept().await {
                Ok((mut stream, _)) => {
                    let aligned_mode = self.aligned_mode;
                    tokio::spawn(async move {
                        handle_connection(stream, aligned_mode).await;
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to accept connection: {e}");
                }
            }
        }
    }
}

async fn handle_connection(mut stream: TcpStream, aligned_mode: bool) {
    // Turn everything into `Response`.
    let response = handle_request(&mut stream, aligned_mode)
        .await
        .unwrap_or_else(|error| Response::Error(error.to_string()));

    // If send() fails, we need to know.
    message::send(&mut stream, &response)
        .await
        .unwrap_or_else(|error| tracing::error!("{error}"));
}

async fn handle_request(
    stream: &mut TcpStream,
    aligned_mode: bool,
) -> Result<Response, InternalError> {
    let request = message::receive::<Request>(stream).await?;
    match request {
        Request::Proof(prover_data) => {
            let batch_proof = prove(Backend::Exec, prover_data.input, aligned_mode)
                .and_then(|output| to_batch_proof(output, aligned_mode))?;
            Ok(Response::Proof(batch_proof))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InternalError {
    #[error("{0}")]
    Message(#[from] MessageError),
    #[error("{0}")]
    Prover(#[from] Box<dyn std::error::Error>),
}
