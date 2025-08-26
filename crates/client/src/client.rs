use crate::{
    error::{Error, Result},
    types::{JobId, ProofResponse, ProverData, SignedBlock, SignedProofResponse},
};
use ethrex_common::types::Block;
use ethrex_rpc::{
    clients::eth::RpcResponse,
    utils::{RpcRequest, RpcRequestId},
};
use futures::{
    FutureExt,
    future::{Fuse, select_ok},
};
use mojave_signature::{Signature, Signer, SigningKey};
use mojave_utils::rpc::types::MojaveRequestMethods;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde_json::{json, to_string};
use std::{pin::Pin, str::FromStr, sync::Arc, time::Duration};
use tokio::time::timeout;

const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(100);
const BACKOFF_FACTOR: u32 = 2;
const MAX_DELAY: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct MojaveClient {
    inner: Arc<MojaveClientInner>,
}

#[derive(Debug)]
struct MojaveClientInner {
    client: reqwest::Client,
    signing_key: SigningKey,
}

impl MojaveClient {
    pub fn new(private_key: &str) -> Result<Self> {
        let signing_key = SigningKey::from_str(private_key)?;
        Ok(Self {
            inner: Arc::new(MojaveClientInner {
                client: reqwest::Client::new(),
                signing_key,
            }),
        })
    }

    /// Sends multiple RPC requests to a list of urls and returns
    /// the first response without waiting for others to finish.
    async fn send_request_race<T>(&self, request: RpcRequest, urls: &[Url]) -> Result<T>
    where
        T: DeserializeOwned,
    {
        if urls.is_empty() {
            return Err(Error::NoRPCUrlsConfigured);
        }

        let requests: Vec<Pin<Box<Fuse<_>>>> = urls
            .iter()
            .map(|url| Box::pin(self.send_request_to_url(&request, url).fuse()))
            .collect();

        let (response, _) = select_ok(requests)
            .await
            .map_err(|error| Error::Custom(format!("All RPC calls failed: {error}")))?;
        Ok(response)
    }

    /// Sends the given RPC request to all configured URLs sequentially.
    /// Returns the response from the first successful request, or the last error if all requests fail.
    #[allow(unused)]
    async fn send_request<T>(&self, request: &RpcRequest, urls: &[Url]) -> Result<T>
    where
        T: DeserializeOwned,
    {
        if urls.is_empty() {
            return Err(Error::NoRPCUrlsConfigured);
        }

        let mut response = Err(Error::Custom("All rpc calls failed".to_string()));

        for url in urls.iter() {
            match self.send_request_to_url(request, url).await {
                Ok(resp) => return Ok(resp),
                Err(e) => response = Err(e),
            }
        }
        response
    }

    async fn send_request_to_url<T>(&self, request: &RpcRequest, url: &Url) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = self
            .inner
            .client
            .post(url.as_ref())
            .header("content-type", "application/json")
            .body(serde_json::to_string(&request)?)
            .send()
            .await?
            .json::<RpcResponse>()
            .await?;

        match response {
            RpcResponse::Success(ok_response) => {
                Ok(serde_json::from_value::<T>(ok_response.result)?)
            }
            RpcResponse::Error(error_response) => Err(Error::Rpc(error_response.error.message)),
        }
    }

    fn is_retryable(error: &Error) -> bool {
        match error {
            Error::Rpc(e) => {
                let error_msg = e.to_string();
                match error_msg.as_str() {
                    msg if msg.starts_with("Internal Error") => true,
                    msg if msg.starts_with("Unknown payload") => true,
                    _ => false,
                }
            }
            Error::TimeOut => true,
            _ => false,
        }
    }

    pub async fn send_request_to_url_with_retry<T>(
        &self,
        request: &RpcRequest,
        url: &Url,
        max_attempts: u64,
        request_timeout: u64,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        if max_attempts < 1 {
            return Err(Error::InvalidMaxAttempts(max_attempts));
        }

        let mut attempts = 0;
        let mut delay = INITIAL_RETRY_DELAY;
        let mut last_error = None;
        while attempts < max_attempts {
            attempts += 1;
            match timeout(
                Duration::from_secs(request_timeout),
                self.send_request_to_url(request, url),
            )
            .await
            {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(e)) => {
                    tracing::error!("Request failed (attempt {}): {}", attempts, e);
                    last_error = Some(e);
                    if Self::is_retryable(last_error.as_ref().unwrap()) {
                        tracing::info!("Retrying request (attempt {})", attempts);
                    } else {
                        return Err(last_error.unwrap());
                    }
                }
                Err(_) => {
                    tracing::error!("Request timed out (attempt {})", attempts);
                    last_error = Some(Error::TimeOut);
                }
            }

            // avoid sleeping on the last attempt
            if attempts < max_attempts {
                tokio::time::sleep(delay).await;
                delay = delay.saturating_mul(BACKOFF_FACTOR);
                if delay > MAX_DELAY {
                    delay = MAX_DELAY;
                }
            }
        }
        Err(last_error.unwrap_or(Error::RetryFailed(max_attempts)))
    }

    pub async fn send_broadcast_block(&self, block: &Block, full_node_urls: &[Url]) -> Result<()> {
        let hash = block.hash();
        let signature: Signature = self.inner.signing_key.sign(&hash)?;
        let verifying_key = self.inner.signing_key.verifying_key();

        let params = SignedBlock {
            block: block.clone(),
            signature,
            verifying_key,
        };

        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: to_string(&MojaveRequestMethods::SendBroadcastBlock)?,
            params: Some(vec![json!(params)]),
        };
        self.send_request_race(request, full_node_urls).await
    }

    pub async fn send_proof_input(
        &self,
        proof_input: &ProverData,
        sequencer_address: &str,
        prover_url: &Url,
    ) -> Result<JobId> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: to_string(&MojaveRequestMethods::SendProofInput)?,
            params: Some(vec![json!(proof_input), json!(sequencer_address)]),
        };
        self.send_request_to_url(&request, prover_url).await
    }

    pub async fn send_proof_response(
        &self,
        proof_response: &ProofResponse,
        sequencer_url: &Url,
    ) -> Result<()> {
        let signature: Signature = self.inner.signing_key.sign(proof_response)?;
        let verifying_key = self.inner.signing_key.verifying_key();

        let params = SignedProofResponse {
            proof_response: proof_response.clone(),
            signature,
            verifying_key,
        };

        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: to_string(&MojaveRequestMethods::SendProofResponse)?,
            params: Some(vec![json!(params)]),
        };
        self.send_request_to_url(&request, sequencer_url).await
    }

    pub async fn get_job_id(&self, prover_url: &Url) -> Result<Vec<JobId>> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: to_string(&MojaveRequestMethods::GetJobId)?,
            params: None,
        };
        self.send_request_to_url(&request, prover_url).await
    }

    pub async fn get_proof(
        &self,
        job_id: JobId,
        prover_url: &Url,
        max_attempts: u64,
        request_timeout: u64,
    ) -> Result<ProofResponse> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: to_string(&MojaveRequestMethods::GetProof)?,
            params: Some(vec![json!(job_id)]),
        };
        self.send_request_to_url_with_retry(&request, prover_url, max_attempts, request_timeout)
            .await
    }
}
