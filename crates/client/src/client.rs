use crate::{
    MojaveClientError,
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
use mojave_signature::{Signature, Signer, SigningKey};
use reqwest::{ClientBuilder, Url};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::{pin::Pin, str::FromStr, sync::Arc, time::Duration};

#[derive(Default)]
pub struct MojaveClientBuilder {
    sequencer_url: Option<Vec<String>>,
    full_node_url: Option<Vec<String>>,
    prover_url: Option<Vec<String>>,
    private_key: Option<String>,
    timeout: Option<Duration>,
}

impl MojaveClientBuilder {
    pub fn sequencer_url(mut self, sequencer_url: &[String]) -> Self {
        self.sequencer_url = Some(sequencer_url.to_owned());
        self
    }

    pub fn full_node_url(mut self, full_node_url: &[String]) -> Self {
        self.full_node_url = Some(full_node_url.to_owned());
        self
    }

    pub fn prover_url(mut self, prover_url: &[String]) -> Self {
        self.prover_url = Some(prover_url.to_owned());
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

    pub fn build(mut self) -> Result<MojaveClient, MojaveClientError> {
        let sequencer_url = self.sequencer_url.take();

        let full_node_url = self.full_node_url.take();

        let prover_url = self.prover_url.take();

        let private_key = self.private_key.take();

        MojaveClient::new(
            sequencer_url,
            full_node_url,
            prover_url,
            private_key,
            self.timeout.unwrap_or(Duration::from_secs(0)),
        )
    }
}

#[derive(Clone)]
pub struct MojaveClient {
    inner: Arc<MojaveClientInner>,
}

struct MojaveClientInner {
    client: reqwest::Client,
    sequencer_url: Option<Vec<Url>>,
    full_node_url: Option<Vec<Url>>,
    prover_url: Option<Vec<Url>>,
    signing_key: Option<SigningKey>,
}

impl MojaveClient {
    pub fn builder() -> MojaveClientBuilder {
        MojaveClientBuilder::default()
    }

    fn parse_urls(urls: Option<Vec<String>>) -> Result<Option<Vec<Url>>, MojaveClientError> {
        match urls {
            Some(urls) => {
                let urls = urls
                    .iter()
                    .map(|url| Url::parse(url))
                    .collect::<Result<Vec<Url>, _>>()
                    .map_err(|error| MojaveClientError::Custom(error.to_string()))?;
                Ok(Some(urls))
            }
            None => Ok(None),
        }
    }

    pub fn new(
        sequencer_url: Option<Vec<String>>,
        full_node_url: Option<Vec<String>>,
        prover_url: Option<Vec<String>>,
        private_key: Option<String>,
        timeout: Duration,
    ) -> Result<Self, MojaveClientError> {
        let http_client = if timeout.is_zero() {
            ClientBuilder::default().build()?
        } else {
            ClientBuilder::default().timeout(timeout).build()?
        };

        let client = MojaveClient {
            inner: Arc::new(MojaveClientInner {
                client: http_client,
                sequencer_url: Self::parse_urls(sequencer_url)?,
                full_node_url: Self::parse_urls(full_node_url)?,
                prover_url: Self::parse_urls(prover_url)?,
                signing_key: match private_key {
                    Some(private_key) => Some(
                        SigningKey::from_str(&private_key)
                            .map_err(|error| MojaveClientError::Custom(error.to_string()))?,
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
            max_retry: 0,
            strategy: Strategy::Sequential,
        }
    }

    async fn send_request<T>(
        &self,
        request: &RpcRequest,
        urls: &[Url],
        max_retry: usize,
        strategy: Strategy,
    ) -> Result<T, MojaveClientError>
    where
        T: DeserializeOwned,
    {
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
    ) -> Result<T, MojaveClientError>
    where
        T: DeserializeOwned,
    {
        let mut response = Err(MojaveClientError::Custom("All rpc calls failed".to_owned()));

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

    async fn send_request_race<T>(
        &self,
        request: &RpcRequest,
        urls: &[Url],
    ) -> Result<T, MojaveClientError>
    where
        T: DeserializeOwned,
    {
        let requests: Vec<Pin<Box<Fuse<_>>>> = urls
            .iter()
            .map(|url| Box::pin(self.send_request_to_url(request, url).fuse()))
            .collect();

        let (response, _) = select_ok(requests)
            .await
            .map_err(|error| MojaveClientError::Custom(format!("All RPC calls failed: {error}")))?;
        Ok(response)
    }

    async fn send_request_to_url<T>(
        &self,
        request: &RpcRequest,
        url: &Url,
    ) -> Result<T, MojaveClientError>
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
            RpcResponse::Error(error_response) => {
                Err(MojaveClientError::RpcError(error_response.error.message))
            }
        }
    }

    pub async fn send_request_to_url_with_retry<T>(
        &self,
        request: &RpcRequest,
        url: &Url,
        max_retry: usize,
    ) -> Result<T, MojaveClientError>
    where
        T: DeserializeOwned,
    {
        let mut error_response = MojaveClientError::Custom("All retry failed".to_owned());

        for _ in 0..max_retry {
            // TODO: Sleep in between request, but this can be implemented with caution
            // because it can be a huge bottleneck especially when it comes to sequential requests.
            match self.send_request_to_url::<T>(request, url).await {
                Ok(response) => return Ok(response),
                Err(error) => error_response = error,
            }
        }
        Err(error_response)
    }
}

pub struct Request<'a> {
    client: &'a MojaveClient,
    urls: Option<&'a [Url]>,
    max_retry: usize,
    strategy: Strategy,
}

impl<'a> Request<'a> {
    pub fn urls(mut self, urls: &'a [Url]) -> Self {
        self.urls = Some(urls);
        self
    }

    pub fn max_retry(mut self, value: usize) -> Self {
        self.max_retry = value;
        self
    }

    pub fn strategy(mut self, strategy: Strategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub async fn send_broadcast_block(&self, block: &Block) -> Result<(), MojaveClientError> {
        let hash = block.hash();
        let signing_key = self
            .client
            .inner
            .signing_key
            .as_ref()
            .ok_or(MojaveClientError::MissingPrivateKey)?;
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
            method: "mojave_sendBroadcastBlock".to_string(),
            params: Some(vec![json!(params)]),
        };

        let urls = match self.urls {
            Some(urls) => urls,
            None => self
                .client
                .inner
                .full_node_url
                .as_ref()
                .ok_or(MojaveClientError::MissingFullNodeUrls)?,
        };

        println!("[DEBUG] urls: {:?}", urls);

        self.client
            .send_request(&request, urls, self.max_retry, self.strategy)
            .await
    }

    pub async fn send_proof_input(
        &self,
        proof_input: &ProverData,
        sequencer_address: &str,
    ) -> Result<JobId, MojaveClientError> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: "mojave_sendProofInput".to_string(),
            params: Some(vec![json!(proof_input), json!(sequencer_address)]),
        };

        let urls = match self.urls {
            Some(urls) => urls,
            None => self
                .client
                .inner
                .prover_url
                .as_ref()
                .ok_or(MojaveClientError::MissingProverUrl)?,
        };

        self.client
            .send_request(&request, urls, self.max_retry, self.strategy)
            .await
    }

    pub async fn get_job_id(&self) -> Result<Vec<JobId>, MojaveClientError> {
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
                .prover_url
                .as_ref()
                .ok_or(MojaveClientError::MissingProverUrl)?,
        };

        self.client
            .send_request(&request, urls, self.max_retry, self.strategy)
            .await
    }

    pub async fn get_proof(&self, job_id: JobId) -> Result<ProofResponse, MojaveClientError> {
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
                .prover_url
                .as_ref()
                .ok_or(MojaveClientError::MissingProverUrl)?,
        };

        self.client
            .send_request(&request, urls, self.max_retry, self.strategy)
            .await
    }

    pub async fn send_proof_response(
        &self,
        proof_response: &ProofResponse,
    ) -> Result<(), MojaveClientError> {
        let signing_key = self
            .client
            .inner
            .signing_key
            .as_ref()
            .ok_or(MojaveClientError::MissingPrivateKey)?;
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
            method: "mojave_sendProofResponse".to_string(),
            params: Some(vec![json!(params)]),
        };

        let urls = match self.urls {
            Some(urls) => urls,
            None => self
                .client
                .inner
                .sequencer_url
                .as_ref()
                .ok_or(MojaveClientError::MissingSequencerUrl)?,
        };

        self.client
            .send_request(&request, urls, self.max_retry, self.strategy)
            .await
    }
}
