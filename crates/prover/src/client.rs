use crate::{
    message::{self, MessageError, Request, Response},
    types::*,
};
use ethrex_l2_common::prover::BatchProof;
use std::time::Duration;
use thiserror;
use tokio::{net::TcpStream, time::timeout};

pub struct ProverClient {
    server_address: String,
    request_timeout: u64,
}

impl ProverClient {
    pub fn new(server_address: &str, request_timeout: u64) -> Self {
        Self {
            server_address: server_address.to_owned(),
            request_timeout,
        }
    }

    async fn request_inner(&mut self, request: Request) -> Result<Response, ProverClientError> {
        let mut stream = TcpStream::connect(&self.server_address).await?;
        message::send(&mut stream, request).await?;
        let response = message::receive::<Response>(&mut stream).await?;
        Ok(response)
    }

    async fn request(&mut self, request: Request) -> Result<Response, ProverClientError> {
        match timeout(
            Duration::from_secs(self.request_timeout),
            self.request_inner(request),
        )
        .await
        {
            Ok(response) => response,
            Err(_) => Err(ProverClientError::TimeOut),
        }
    }

    pub async fn get_proof(&mut self, data: ProverData) -> Result<BatchProof, ProverClientError> {
        match self.request(Request::Proof(data)).await? {
            Response::Proof(proof) => Ok(proof),
            Response::Error(error) => Err(ProverClientError::Internal(error)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProverClientError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Message error: {0}")]
    Message(#[from] MessageError),
    #[error("Internal server error: {0}")]
    Internal(String),
    #[error("Unexpected error: {0}")]
    Unexpected(String),
    #[error("Connection timed out")]
    TimeOut,
}
