use crate::{
    message::{self, MessageError, Request, Response},
    types::*,
};
use ethrex_l2_common::prover::BatchProof;
use std::time::Duration;
use tokio::{net::TcpStream, time::timeout};
use tracing::error;

const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(100);
const BACKOFF_FACTOR: u32 = 2;
const MAX_DELAY: Duration = Duration::from_secs(30);

pub struct ProverClient {
    server_address: String,
    request_timeout: u64,
    max_number_of_retries: u64,
}

impl ProverClient {
    pub fn new(server_address: &str, request_timeout: u64, max_number_of_retries: u64) -> Self {
        Self {
            server_address: server_address.to_owned(),
            request_timeout,
            max_number_of_retries,
        }
    }

    fn is_retryable(error: &ProverClientError) -> bool {
        !matches!(
            error,
            ProverClientError::Message(MessageError::MessageTooLarge(_, _))
                | ProverClientError::Message(MessageError::Serialize(_))
                | ProverClientError::Message(MessageError::Deserialize(_))
                | ProverClientError::Internal(_)
        )
    }

    async fn request_inner(&mut self, request: &Request) -> Result<Response, ProverClientError> {
        let mut stream = TcpStream::connect(&self.server_address).await?;
        message::send(&mut stream, request).await?;
        let response = message::receive::<Response>(&mut stream).await?;
        Ok(response)
    }

    async fn request(&mut self, request: &Request) -> Result<Response, ProverClientError> {
        let mut number_of_retries = 0;
        let mut delay = INITIAL_RETRY_DELAY;
        while number_of_retries < self.max_number_of_retries {
            number_of_retries += 1;
            match timeout(
                Duration::from_secs(self.request_timeout),
                self.request_inner(request),
            )
            .await
            {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(e)) => {
                    if Self::is_retryable(&e) {
                        tracing::info!("Retrying request (attempt {})", number_of_retries);
                    } else {
                        return Err(e);
                    }
                    tracing::error!(
                        "Prover request failed (attempt {}): {}",
                        number_of_retries,
                        e
                    );
                }
                Err(_) => {
                    tracing::error!("Prover request timed out (attempt {})", number_of_retries);
                }
            }

            // avoid sleeping on the last attempt
            if number_of_retries < self.max_number_of_retries {
                tokio::time::sleep(delay).await;
                delay *= BACKOFF_FACTOR;
                if delay > MAX_DELAY {
                    delay = MAX_DELAY;
                }
            }
        }
        Err(ProverClientError::RetryFailed(self.max_number_of_retries))
    }

    pub async fn get_proof(&mut self, data: ProverData) -> Result<BatchProof, ProverClientError> {
        match self.request(&Request::Proof(data)).await? {
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
    #[error("Retry failed after {0} attempts")]
    RetryFailed(u64),
}
