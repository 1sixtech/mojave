use ethrex_l2_common::prover::BatchProof;
use ethrex_prover_lib::{prove, to_batch_proof};
use tokio::net::{TcpListener, TcpStream};

use tracing::error;

use crate::{
    message::Message,
    request::ProverData,
    response::{Response, ResponseError},
};

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
                Ok((stream, addr)) => {
                    let aligned_mode = self.aligned_mode;
                    tokio::spawn(async move {
                        handle_connection(stream, aligned_mode).await;
                    });
                }
                Err(e) => {
                    error!("error accepting connection: {e}");
                }
            }
        }
    }
}

async fn handle_connection(mut stream: TcpStream, aligned_mode: bool) {
    if let Err(error) = async {
        
    let prover_data = receive_data(&mut stream).await?;
    let batch_proof = generate_proof(prover_data, aligned_mode).await?;
    send_response(&mut stream, batch_proof).await?;
    Ok(())

    }.await {
        send_err(&mut stream, error).await;
    }
}

async fn receive_data(stream: &mut TcpStream) -> Result<ProverData, ResponseError> {
    match Message::receive::<ProverData>(stream).await {
        Ok(data) => Ok(data),
        Err(e) => {
            error!("Error while receiving data from stream: {e}");
            Err(ResponseError::StreamError(e.to_string()))
        }
    }
}

async fn generate_proof(prover_data: ProverData, aligned_mode: bool) -> Result<BatchProof, ResponseError> {
    prove(prover_data.input, aligned_mode)
    .and_then(|output| to_batch_proof(output, aligned_mode))
    .map_err(|e| {
        error!("Proving error: {e}");
        ResponseError::ProofError(e.to_string())
    })
}

async fn send_response(
    stream: &mut TcpStream,
    batch_proof: BatchProof,
) -> Result<(), ResponseError> {
    match Message::send(
        stream,
        &Response::Proof(batch_proof),
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(error) => {
            error!("Error while write stream: {error}");
            Err(ResponseError::StreamError(error.to_string()))
        }
    }
}

async fn send_err(stream: &mut TcpStream, error: ResponseError) {
    if let Err(error) = Message::send(stream, &Response::Error(error)).await {
        error!("Fail to send error response: {error}")
    };
}
