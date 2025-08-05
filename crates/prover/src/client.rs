use std::time::Duration;

use ethrex_l2_common::prover::BatchProof;
use thiserror;
use tokio::{net::TcpStream, time::timeout};

use crate::{
    message::{Message, MessageError},
    request::{ProverData, Request},
    response::Response,
};

pub struct ProverClient {
    client: TcpStream,
    timeout: u64,
}

impl ProverClient {
    pub async fn new(addr: String, timeout: u64) -> Result<Self, ProverClientError> {
        let client = TcpStream::connect(addr).await?;
        Ok(Self { client, timeout })
    }

    pub async fn get_proof(&mut self, data: ProverData) -> Result<BatchProof, ProverClientError> {
        let future = async {
            Message::send(&mut self.client, &Request::Proof(data)).await?;
            let response: Response = Message::receive(&mut self.client).await?;

            match response {
                Response::Proof(proof) => Ok(proof),
                Response::Error(error) => Err(ProverClientError::Internal(Box::new(error))),
                others => Err(ProverClientError::Unexpected(format!(
                    "Unexpected response: {:?}",
                    others
                ))),
            }
        };

        timeout(Duration::from_secs(self.timeout), future)
            .await
            .map_err(|_| ProverClientError::Timeout)?
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProverClientError {
    #[error("Connection error: {0}")]
    Connection(#[from] std::io::Error),
    #[error("Prover server error: {0}")]
    Internal(Box<dyn std::error::Error>),
    #[error("Prover communication error: {0}")]
    InternalCommunication(#[from] MessageError),
    #[error("Timeout error")]
    Timeout,
    #[error("Unexpected error: {0}")]
    Unexpected(String),
}
