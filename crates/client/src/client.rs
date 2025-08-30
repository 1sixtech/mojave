use crate::{
    error::{Error, Result},
    types::{JobId, ProofResponse, ProverData, SignedBlock, SignedProofResponse, Strategy},
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
use mojave_signature::{
    SigningKey,
    types::{Signature, Signer},
};
use mojave_utils::rpc::types::MojaveRequestMethods;
use reqwest::{ClientBuilder, Url};
use serde::de::DeserializeOwned;
use serde_json::{json, to_string};
use std::{pin::Pin, str::FromStr, sync::Arc, time::Duration};

const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(100);
const BACKOFF_FACTOR: u32 = 2;
const MAX_DELAY: Duration = Duration::from_secs(30);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Default)]
pub struct MojaveClientBuilder {
    sequencer_urls: Option<Vec<String>>,
    full_node_urls: Option<Vec<String>>,
    prover_urls: Option<Vec<String>>,
    private_key: Option<String>,
    timeout: Option<Duration>,
}

impl MojaveClientBuilder {
    pub fn sequencer_urls(mut self, sequencer_urls: &[String]) -> Self {
        self.sequencer_urls = Some(sequencer_urls.to_owned());
        self
    }

    pub fn full_node_urls(mut self, full_node_urls: &[String]) -> Self {
        self.full_node_urls = Some(full_node_urls.to_owned());
        self
    }

    pub fn prover_urls(mut self, prover_urls: &[String]) -> Self {
        self.prover_urls = Some(prover_urls.to_owned());
        self
    }

    pub fn private_key(mut self, private_key: String) -> Self {
        self.private_key = Some(private_key);
        self
    }

    /// Enables a total request timeout.
    ///
    /// The timeout is applied from when the request starts connecting until the
    /// response body has finished. Also considered a total deadline.
    ///
    /// Default is no timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build(mut self) -> Result<MojaveClient> {
        let sequencer_urls = self.sequencer_urls.take();
        let full_node_urls = self.full_node_urls.take();
        let prover_urls = self.prover_urls.take();
        let private_key = self.private_key.take();
        let timeout = self.timeout.take();

        MojaveClient::new(
            sequencer_urls,
            full_node_urls,
            prover_urls,
            private_key,
            timeout,
        )
    }
}

#[derive(Clone)]
pub struct MojaveClient {
    inner: Arc<MojaveClientInner>,
}

struct MojaveClientInner {
    client: reqwest::Client,
    sequencer_urls: Option<Vec<Url>>,
    full_node_urls: Option<Vec<Url>>,
    prover_urls: Option<Vec<Url>>,
    signing_key: Option<SigningKey>,
}

impl MojaveClient {
    pub fn builder() -> MojaveClientBuilder {
        MojaveClientBuilder::default()
    }

    fn parse_urls(urls: Option<Vec<String>>) -> Result<Option<Vec<Url>>> {
        match urls {
            Some(urls) => {
                let urls = urls
                    .iter()
                    .map(|url| Url::parse(url))
                    .collect::<core::result::Result<Vec<Url>, _>>()
                    .map_err(|error| Error::Custom(error.to_string()))?;
                Ok(Some(urls))
            }
            None => Ok(None),
        }
    }

    pub fn new(
        sequencer_urls: Option<Vec<String>>,
        full_node_urls: Option<Vec<String>>,
        prover_urls: Option<Vec<String>>,
        private_key: Option<String>,
        timeout: Option<Duration>,
    ) -> Result<Self> {
        let http_client = ClientBuilder::new()
            .timeout(timeout.unwrap_or(DEFAULT_TIMEOUT))
            .build()?;

        let client = MojaveClient {
            inner: Arc::new(MojaveClientInner {
                client: http_client,
                sequencer_urls: Self::parse_urls(sequencer_urls)?,
                full_node_urls: Self::parse_urls(full_node_urls)?,
                prover_urls: Self::parse_urls(prover_urls)?,
                signing_key: match private_key {
                    Some(private_key) => Some(
                        SigningKey::from_str(&private_key)
                            .map_err(|error| Error::Custom(error.to_string()))?,
                    ),
                    None => None,
                },
            }),
        };
        Ok(client)
    }

    pub fn request(&self) -> Request<'_> {
        Request {
            client: self,
            urls: None,
            max_retry: Some(1),
            strategy: Strategy::Sequential,
        }
    }

    async fn send_request<T>(
        &self,
        request: &RpcRequest,
        urls: &[Url],
        max_retry: usize,
        strategy: Strategy,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        if urls.is_empty() {
            return Err(Error::NoRPCUrlsConfigured);
        }

        match strategy {
            Strategy::Sequential => self.send_request_sequential(request, urls, max_retry).await,
            Strategy::Race => self.send_request_race(request, urls).await,
        }
    }

    async fn send_request_sequential<T>(
        &self,
        request: &RpcRequest,
        urls: &[Url],
        max_retry: usize,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut response = Err(Error::Custom("All rpc calls failed".to_owned()));

        for url in urls.iter() {
            match self
                .send_request_to_url_with_retry(request, url, max_retry)
                .await
            {
                Ok(response) => return Ok(response),
                Err(error) => response = Err(error),
            }
        }
        response
    }

    async fn send_request_race<T>(&self, request: &RpcRequest, urls: &[Url]) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let requests: Vec<Pin<Box<Fuse<_>>>> = urls
            .iter()
            .map(|url| Box::pin(self.send_request_to_url(request, url).fuse()))
            .collect();

        let (response, _) = select_ok(requests)
            .await
            .map_err(|error| Error::Custom(format!("All RPC calls failed: {error}")))?;
        Ok(response)
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
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    Error::TimeOut
                } else {
                    Error::Custom(error.to_string())
                }
            })?
            .json::<RpcResponse>()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    Error::TimeOut
                } else {
                    Error::Custom(error.to_string())
                }
            })?;

        match response {
            RpcResponse::Success(ok_response) => {
                Ok(serde_json::from_value::<T>(ok_response.result)?)
            }
            RpcResponse::Error(error_response) => Err(Error::Rpc(error_response.error.message)),
        }
    }

    fn is_retryable(error: &Error) -> bool {
        matches!(error, Error::TimeOut)
    }

    pub async fn send_request_to_url_with_retry<T>(
        &self,
        request: &RpcRequest,
        url: &Url,
        max_retry: usize,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut retry = 0;
        let mut delay = INITIAL_RETRY_DELAY;
        let mut last_error = None;
        while retry < max_retry {
            retry += 1;
            match self.send_request_to_url(request, url).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    tracing::error!("Request failed (attempt {}): {}", retry, e);
                    last_error = Some(e);
                    if Self::is_retryable(last_error.as_ref().unwrap()) {
                        tracing::info!("Retrying request (attempt {})", retry);
                    } else {
                        return Err(last_error.unwrap());
                    }
                }
            }

            // avoid sleeping on the last attempt
            if retry < max_retry {
                tokio::time::sleep(delay).await;
                delay = delay.saturating_mul(BACKOFF_FACTOR);
                if delay > MAX_DELAY {
                    delay = MAX_DELAY;
                }
            }
        }
        Err(last_error.unwrap_or(Error::RetryFailed(max_retry as u64)))
    }
}

pub struct Request<'a> {
    client: &'a MojaveClient,
    urls: Option<&'a [Url]>,
    max_retry: Option<usize>,
    strategy: Strategy,
}

impl<'a> Request<'a> {
    pub fn urls(mut self, urls: &'a [Url]) -> Self {
        self.urls = Some(urls);
        self
    }

    pub fn max_retry(mut self, value: usize) -> Self {
        self.max_retry = Some(value);
        self
    }

    pub fn strategy(mut self, strategy: Strategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub async fn send_broadcast_block(&self, block: &Block) -> Result<()> {
        let hash = block.hash();
        let signing_key = self
            .client
            .inner
            .signing_key
            .as_ref()
            .ok_or(Error::MissingPrivateKey)?;
        let signature: Signature = signing_key.sign(&hash)?;
        let verifying_key = signing_key.verifying_key();

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

        let urls = match self.urls {
            Some(urls) => urls,
            None => self
                .client
                .inner
                .full_node_urls
                .as_ref()
                .ok_or(Error::MissingFullNodeUrls)?,
        };

        self.client
            .send_request(&request, urls, self.max_retry.unwrap_or(1), self.strategy)
            .await
    }

    pub async fn send_proof_input(
        &self,
        proof_input: &ProverData,
        sequencer_address: &str,
    ) -> Result<JobId> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: to_string(&MojaveRequestMethods::SendProofInput)?,
            params: Some(vec![json!(proof_input), json!(sequencer_address)]),
        };

        let urls = match self.urls {
            Some(urls) => urls,
            None => self
                .client
                .inner
                .prover_urls
                .as_ref()
                .ok_or(Error::MissingProverUrl)?,
        };

        self.client
            .send_request(&request, urls, self.max_retry.unwrap_or(1), self.strategy)
            .await
    }

    pub async fn get_job_id(&self) -> Result<Vec<JobId>> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: "mojave_getJobId".to_string(),
            params: None,
        };

        let urls = match self.urls {
            Some(urls) => urls,
            None => self
                .client
                .inner
                .prover_urls
                .as_ref()
                .ok_or(Error::MissingProverUrl)?,
        };

        self.client
            .send_request(&request, urls, self.max_retry.unwrap_or(1), self.strategy)
            .await
    }

    pub async fn get_proof(&self, job_id: JobId) -> Result<ProofResponse> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: "mojave_getProof".to_string(),
            params: Some(vec![json!(job_id)]),
        };

        let urls = match self.urls {
            Some(urls) => urls,
            None => self
                .client
                .inner
                .prover_urls
                .as_ref()
                .ok_or(Error::MissingProverUrl)?,
        };

        self.client
            .send_request(&request, urls, self.max_retry.unwrap_or(1), self.strategy)
            .await
    }

    pub async fn send_proof_response(&self, proof_response: &ProofResponse) -> Result<()> {
        let signing_key = self
            .client
            .inner
            .signing_key
            .as_ref()
            .ok_or(Error::MissingPrivateKey)?;
        let signature: Signature = signing_key.sign(proof_response)?;
        let verifying_key = signing_key.verifying_key();

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

        let urls = match self.urls {
            Some(urls) => urls,
            None => self
                .client
                .inner
                .sequencer_urls
                .as_ref()
                .ok_or(Error::MissingSequencerUrl)?,
        };

        self.client
            .send_request(&request, urls, self.max_retry.unwrap_or(1), self.strategy)
            .await
    }
}
