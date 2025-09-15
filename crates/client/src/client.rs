use crate::{
    constants::DEFAULT_TIMEOUT,
    error::{Error, Result},
    request_builder::RequestBuilder,
    retry_config::RetryConfig,
    types::{JobId, ProofResponse, ProverData},
    utils::parse_urls,
};
use mojave_signature::SigningKey;
use reqwest::{ClientBuilder, Url};
use std::{str::FromStr, sync::Arc, time::Duration};

#[derive(Default)]
pub struct MojaveClientBuilder {
    sequencer_urls: Vec<String>,
    full_node_urls: Vec<String>,
    prover_urls: Vec<String>,
    private_key: Option<String>,
    timeout: Duration,
    retry_config: RetryConfig,
}

impl MojaveClientBuilder {
    pub fn new() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            retry_config: RetryConfig::default(),
            ..Default::default()
        }
    }

    pub fn sequencer_urls<I, S>(mut self, urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.sequencer_urls = urls.into_iter().map(Into::into).collect();
        self
    }

    pub fn full_node_urls<I, S>(mut self, urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.full_node_urls = urls.into_iter().map(Into::into).collect();
        self
    }

    pub fn prover_urls<I, S>(mut self, urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.prover_urls = urls.into_iter().map(Into::into).collect();
        self
    }

    pub fn private_key<S: Into<String>>(mut self, private_key: S) -> Self {
        self.private_key = Some(private_key.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    pub fn build(self) -> Result<MojaveClient> {
        let http_client = ClientBuilder::new().timeout(self.timeout).build()?;

        let signing_key = self
            .private_key
            .as_ref()
            .map(|key| SigningKey::from_str(key))
            .transpose()
            .map_err(|e| Error::Custom(e.to_string()))?;

        Ok(MojaveClient {
            inner: Arc::new(MojaveClientInner {
                client: http_client,
                sequencer_urls: parse_urls(self.sequencer_urls)?,
                full_node_urls: parse_urls(self.full_node_urls)?,
                prover_urls: parse_urls(self.prover_urls)?,
                retry_config: self.retry_config,
                _signing_key: signing_key,
            }),
        })
    }
}

#[derive(Clone)]
pub struct MojaveClient {
    pub(crate) inner: Arc<MojaveClientInner>,
}

pub(crate) struct MojaveClientInner {
    pub(crate) client: reqwest::Client,
    pub(crate) sequencer_urls: Vec<Url>,
    pub(crate) full_node_urls: Vec<Url>,
    pub(crate) prover_urls: Vec<Url>,
    pub(crate) retry_config: RetryConfig,
    _signing_key: Option<SigningKey>,
}

impl MojaveClient {
    pub fn builder() -> MojaveClientBuilder {
        MojaveClientBuilder::new()
    }

    pub fn sequencer_urls(&self) -> &[Url] {
        &self.inner.sequencer_urls
    }

    pub fn full_node_urls(&self) -> &[Url] {
        &self.inner.full_node_urls
    }

    pub fn prover_urls(&self) -> &[Url] {
        &self.inner.prover_urls
    }

    pub fn request(&self) -> RequestBuilder<'_> {
        RequestBuilder::new(self)
    }
    pub async fn send_proof_input(
        &self,
        proof_input: &ProverData,
        sequencer_address: &str,
    ) -> Result<JobId> {
        self.request()
            .with_provers()
            .send_proof_input(proof_input, sequencer_address)
            .await
    }

    pub async fn get_pending_job_ids(&self) -> Result<Vec<JobId>> {
        self.request().with_provers().get_pending_job_ids().await
    }

    pub async fn get_proof(&self, job_id: JobId) -> Result<ProofResponse> {
        self.request().with_provers().get_proof(job_id).await
    }
}
