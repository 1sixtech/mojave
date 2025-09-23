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

#[derive(Clone, Debug)]
pub struct MojaveClient {
    pub(crate) inner: Arc<MojaveClientInner>,
}

#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use mojave_rpc_core::{RpcErr, RpcRequest, types::Namespace};
    use mojave_rpc_server::{RpcRegistry, RpcService};
    use serde_json::json;
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        time::Duration,
    };
    use tokio::{net::TcpStream, task::JoinHandle};

    #[derive(Clone)]
    enum Behavior {
        Ok(&'static str, serde_json::Value), // {"result":...} for method pattern
        SleepThenOk(Duration, &'static str, serde_json::Value), // after sleep, {"result":...} for method pattern
        JsonRpcInternalError(&'static str),                     // {"error": {"code": -32603, ...}}
    }

    struct TestRpc {
        base_url: String,
        task: JoinHandle<()>,
    }

    impl TestRpc {
        pub async fn spawn(behavior: Behavior) -> Self {
            let mut reg: RpcRegistry<()> = RpcRegistry::new();
            reg.register_fallback(Namespace::Mojave, move |req: &RpcRequest, _| {
                let b = behavior.clone();
                let method = serde_json::from_str::<String>(&req.method).unwrap();
                Box::pin(async move {
                    match b {
                        Behavior::Ok(matcher, val) => {
                            if matcher == method {
                                Ok(val)
                            } else {
                                Err(RpcErr::Internal(format!(
                                    "Method '{method}' did not match expected '{matcher}'",
                                )))
                            }
                        }
                        Behavior::SleepThenOk(duration, matcher, val) => {
                            if matcher == method {
                                tokio::time::sleep(duration).await;
                                Ok(val)
                            } else {
                                Err(RpcErr::Internal(format!(
                                    "Method '{method}' did not match expected '{matcher}'",
                                )))
                            }
                        }
                        Behavior::JsonRpcInternalError(msg) => {
                            Err(RpcErr::Internal(msg.to_string()))
                        }
                    }
                })
            });

            let service = RpcService::new((), reg);

            let port = pick_free_port().unwrap_or(0);
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            let task = tokio::spawn(async move {
                let app = service.router();
                axum::serve(listener, app)
                    .with_graceful_shutdown(ethrex_rpc::shutdown_signal())
                    .await
                    .unwrap()
            });

            let base_url = format!("http://{}:{}", addr.ip(), addr.port());

            wait_until_listen(addr, Duration::from_millis(500)).await;

            Self { base_url, task }
        }

        pub fn url(&self) -> &str {
            &self.base_url
        }
    }

    impl Drop for TestRpc {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    fn pick_free_port() -> Option<u16> {
        std::net::TcpListener::bind("127.0.0.1:0")
            .ok()
            .and_then(|l| l.local_addr().ok())
            .map(|addr| addr.port())
    }

    async fn wait_until_listen(addr: SocketAddr, timeout: Duration) {
        let mut waited = Duration::ZERO;
        let step = Duration::from_millis(15);
        while waited < timeout {
            if TcpStream::connect(addr).await.is_ok() {
                break;
            }
            tokio::time::sleep(step).await;
            waited += step;
        }
    }

    #[test]
    fn invalid_url_in_builder_returns_error() {
        let res = MojaveClient::builder()
            .prover_urls(vec![
                "http://127.0.0.1:1",
                "http://:://not-valid",
                "not-url",
            ])
            .build();

        // This error requires that each url within the vector be propagated as an error individually.
        // and not just a custom "empty host" error but a specific InvalidUrl error.
        assert!(matches!(res, Err(Error::Custom(_))));
    }

    #[test]
    fn invalid_private_key_is_error() {
        let res = MojaveClient::builder()
            .prover_urls(vec!["http://127.0.0.1:1"])
            .private_key("0x-not-hex")
            .build();

        // Needs to be specific error not just custom.
        assert!(matches!(res, Err(Error::Custom(_))));
    }

    #[test]
    fn retry_config_is_applied() {
        let cfg = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(7),
            backoff_factor: 2,
            max_delay: Duration::from_millis(50),
        };
        let client = MojaveClient::builder()
            .prover_urls(vec!["http://127.0.0.1:1"])
            .retry_config(cfg.clone())
            .build()
            .unwrap();

        assert_eq!(client.inner.retry_config.max_retries, cfg.max_retries);
        assert_eq!(client.inner.retry_config.initial_delay, cfg.initial_delay);
        assert_eq!(client.inner.retry_config.backoff_factor, cfg.backoff_factor);
        assert_eq!(client.inner.retry_config.max_delay, cfg.max_delay);
    }

    #[tokio::test]
    async fn builder_sets_urls_from_accessors() {
        let p1 = "http://127.0.0.1:12345";
        let s1 = "http://127.0.0.1:23456";
        let f1 = "http://127.0.0.1:34567";

        let client = MojaveClient::builder()
            .prover_urls(vec![p1])
            .sequencer_urls(vec![s1])
            .full_node_urls(vec![f1])
            .build()
            .unwrap();

        assert_eq!(client.prover_urls(), &[Url::from_str(p1).unwrap()]);
        assert_eq!(client.sequencer_urls(), &[Url::from_str(s1).unwrap()]);
        assert_eq!(client.full_node_urls(), &[Url::from_str(f1).unwrap()]);
    }

    #[tokio::test]
    async fn missing_prover_url_is_error_for_get_pending_job_ids() {
        let client = MojaveClient::builder()
            .timeout(Duration::from_millis(200))
            .build()
            .unwrap();

        let err = client.get_pending_job_ids().await.unwrap_err();
        assert!(matches!(err, Error::NoRPCUrlsConfigured));
    }

    #[tokio::test]
    async fn get_pending_job_ids_success_with_empty_array() {
        let server = TestRpc::spawn(Behavior::Ok("moj_getPendingJobIds", json!([]))).await;

        let client = MojaveClient::builder()
            .prover_urls(vec![server.url().to_string()])
            .timeout(Duration::from_millis(500))
            .build()
            .unwrap();

        let res = client.get_pending_job_ids().await;

        assert!(res.is_ok());
        assert!(res.unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_pending_job_ids_jsonrpc_error_is_propagated() {
        let server = TestRpc::spawn(Behavior::JsonRpcInternalError("boom")).await;

        let client = MojaveClient::builder()
            .prover_urls(vec![server.url().to_string()])
            .timeout(Duration::from_millis(500))
            .build()
            .unwrap();

        let res = client.get_pending_job_ids().await;
        let s = format!("{res:?}").to_lowercase();

        assert!(s.contains("boom"));
    }

    #[tokio::test]
    async fn request_timeout_errors_without_strategy_knobs() {
        let slow = TestRpc::spawn(Behavior::SleepThenOk(
            Duration::from_millis(250),
            "moj_getPendingJobIds",
            json!({}),
        ))
        .await;

        let client = MojaveClient::builder()
            .prover_urls(vec![slow.url().to_string()])
            .timeout(Duration::from_millis(50))
            .build()
            .unwrap();

        let err = client.get_pending_job_ids().await.unwrap_err();
        let s = format!("{err:?}").to_lowercase();

        assert!(s.contains("timedout"));
    }

    #[tokio::test]
    async fn get_proof_success() {
        use crate::types::{ProofResponse, ProofResult};

        let expected = ProofResponse {
            job_id: "job-1".into(),
            batch_number: 7,
            result: ProofResult::Error("dummy".to_string()),
        };
        let server = TestRpc::spawn(Behavior::Ok(
            "moj_getProof",
            serde_json::to_value(&expected).unwrap(),
        ))
        .await;

        let client = MojaveClient::builder()
            .prover_urls(vec![server.url().to_string()])
            .timeout(std::time::Duration::from_millis(500))
            .build()
            .unwrap();

        let got = client.get_proof(expected.job_id.clone()).await.unwrap();

        assert_eq!(got.job_id, expected.job_id);
        assert_eq!(got.batch_number, expected.batch_number);
        assert!(matches!(got.result, ProofResult::Error(_)));
        assert_eq!(
            format!("{:?}", got.result),
            format!("{:?}", expected.result)
        );
    }

    #[tokio::test]
    async fn get_proof_failed_with_delay() {
        use crate::types::{ProofResponse, ProofResult};

        let expected = ProofResponse {
            job_id: "job-1".into(),
            batch_number: 7,
            result: ProofResult::Error("dummy".to_string()),
        };
        let server = TestRpc::spawn(Behavior::SleepThenOk(
            Duration::from_millis(100),
            "moj_getProof",
            serde_json::to_value(&expected).unwrap(),
        ))
        .await;

        let client = MojaveClient::builder()
            .prover_urls(vec![server.url().to_string()])
            .timeout(std::time::Duration::from_millis(50))
            .build()
            .unwrap();

        let got = client.get_proof(expected.job_id.clone()).await.unwrap_err();

        let s = format!("{got:?}").to_lowercase();
        assert!(s.contains("timedout"));
    }

    #[tokio::test]
    async fn send_proof_input_ok() {
        let service = TestRpc::spawn(Behavior::Ok("moj_sendProofInput", json!("job-42"))).await;

        let client = MojaveClient::builder()
            .prover_urls(vec![service.url().to_string()])
            .timeout(Duration::from_millis(500))
            .build()
            .unwrap();

        let proof_in = ProverData {
            batch_number: 1,
            input: guest_program::input::ProgramInput::default(),
        };

        let job_id = client.send_proof_input(&proof_in, "0xabc").await.unwrap();

        assert_eq!(job_id, "job-42".into());
    }

    #[tokio::test]
    async fn send_proof_input_failed_with_delay() {
        let service = TestRpc::spawn(Behavior::SleepThenOk(
            Duration::from_millis(100),
            "moj_sendProofInput",
            json!("job-42"),
        ))
        .await;

        let client = MojaveClient::builder()
            .prover_urls(vec![service.url().to_string()])
            .timeout(Duration::from_millis(50))
            .build()
            .unwrap();

        let proof_in = ProverData {
            batch_number: 1,
            input: guest_program::input::ProgramInput::default(),
        };
        let res = client.send_proof_input(&proof_in, "0xabc").await;
        let err = res.unwrap_err();
        let s = format!("{err:?}").to_lowercase();

        assert!(s.contains("timedout"));
    }
}
