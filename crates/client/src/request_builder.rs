use ethrex_rpc::utils::RpcRequest;
use mojave_utils::rpc::types::MojaveRequestMethods;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::{
    MojaveClient,
    error::{Error, Result},
    retry_config::RetryConfig,
    types::{JobId, ProofResponse, ProverData, Strategy},
    utils::{create_rpc_request, send_request_race, send_request_sequential},
};

pub struct RequestBuilder<'a> {
    client: &'a MojaveClient,
    target_urls: Option<&'a [Url]>,
    strategy: Strategy,
    retry_config: Option<RetryConfig>,
}

impl<'a> RequestBuilder<'a> {
    pub fn new(client: &'a MojaveClient) -> Self {
        Self {
            client,
            target_urls: None,
            strategy: Strategy::Sequential,
            retry_config: None,
        }
    }

    pub fn with_sequencers(mut self) -> Self {
        self.target_urls = Some(&self.client.inner.sequencer_urls);
        self
    }

    pub fn with_full_nodes(mut self) -> Self {
        self.target_urls = Some(&self.client.inner.full_node_urls);
        self
    }

    pub fn with_provers(mut self) -> Self {
        self.target_urls = Some(&self.client.inner.prover_urls);
        self
    }

    pub fn with_urls(mut self, urls: &'a [Url]) -> Self {
        self.target_urls = Some(urls);
        self
    }

    pub fn with_strategy(mut self, strategy: Strategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    fn effective_retry_config(&self) -> &RetryConfig {
        self.retry_config
            .as_ref()
            .unwrap_or(&self.client.inner.retry_config)
    }

    fn get_target_urls(&self) -> Result<&[Url]> {
        match self.target_urls {
            Some(urls) if !urls.is_empty() => Ok(urls),
            Some(_) => Err(Error::NoRPCUrlsConfigured),
            None => Err(Error::NoRPCUrlsConfigured),
        }
    }

    async fn send_rpc_request<T>(&self, request: &RpcRequest) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let urls = self.get_target_urls()?;
        let retry_config = self.effective_retry_config();

        match self.strategy {
            Strategy::Sequential => {
                send_request_sequential(&self.client.inner.client, request, urls, retry_config)
                    .await
            }
            Strategy::Race => send_request_race(&self.client.inner.client, request, urls).await,
        }
    }

    pub async fn send_proof_input(
        self,
        proof_input: &ProverData,
        sequencer_address: &str,
    ) -> Result<JobId> {
        let request = create_rpc_request(
            MojaveRequestMethods::SendProofInput,
            Some(vec![json!(proof_input), json!(sequencer_address)]),
        )?;

        self.send_rpc_request(&request).await
    }

    pub async fn get_pending_job_ids(self) -> Result<Vec<JobId>> {
        let request = create_rpc_request(MojaveRequestMethods::GetPendingJobIds, None)?;

        self.send_rpc_request(&request).await
    }

    pub async fn get_proof(self, job_id: JobId) -> Result<ProofResponse> {
        let request =
            create_rpc_request(MojaveRequestMethods::GetProof, Some(vec![json!(job_id)]))?;

        self.send_rpc_request(&request).await
    }
}
