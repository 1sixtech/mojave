use crate::{MojaveClientError, types::SignedBlock};
use ethrex_common::types::Block;
use ethrex_l2_common::prover::BatchProof;
use ethrex_rpc::{
    clients::eth::RpcResponse,
    utils::{RpcRequest, RpcRequestId},
};
use futures::{
    FutureExt,
    future::{Fuse, select_ok},
};
use mojave_prover::ProverData;
use mojave_signature::{Signature, Signer, SigningKey};
use reqwest::Url;
use serde_json::json;
use std::{pin::Pin, str::FromStr, sync::Arc};

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
    pub fn new(private_key: &str) -> Result<Self, MojaveClientError> {
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
    async fn send_request_race(
        &self,
        request: RpcRequest,
        urls: Vec<Url>,
    ) -> Result<RpcResponse, MojaveClientError> {
        if urls.is_empty() {
            return Err(MojaveClientError::NoRPCUrlsConfigured);
        }

        let requests: Vec<Pin<Box<Fuse<_>>>> = urls
            .iter()
            .map(|url| Box::pin(self.send_request_to_url(url, &request).fuse()))
            .collect();

        let (response, _) = select_ok(requests)
            .await
            .map_err(|error| MojaveClientError::Custom(format!("All RPC calls failed: {error}")))?;
        Ok(response)
    }

    /// Sends the given RPC request to all configured URLs sequentially.
    /// Returns the response from the first successful request, or the last error if all requests fail.
    #[allow(unused)]
    async fn send_request(
        &self,
        request: RpcRequest,
        urls: Vec<Url>,
    ) -> Result<RpcResponse, MojaveClientError> {
        if urls.is_empty() {
            return Err(MojaveClientError::NoRPCUrlsConfigured);
        }

        let mut response = Err(MojaveClientError::Custom(
            "All rpc calls failed".to_string(),
        ));

        for url in urls.iter() {
            match self.send_request_to_url(url, &request).await {
                Ok(resp) => return Ok(resp),
                Err(e) => response = Err(e),
            }
        }
        response
    }

    async fn send_request_to_url(
        &self,
        url: &Url,
        request: &RpcRequest,
    ) -> Result<RpcResponse, MojaveClientError> {
        self.inner
            .client
            .post(url.as_ref())
            .header("content-type", "application/json")
            .body(serde_json::ser::to_string(&request).map_err(|error| {
                MojaveClientError::FailedToSerializeRequestBody(format!("{error}: {request:?}"))
            })?)
            .send()
            .await?
            .json::<RpcResponse>()
            .await
            .map_err(MojaveClientError::from)
    }

    pub async fn send_broadcast_block(
        &self,
        block: &Block,
        sequencer_parsed_urls: Vec<Url>,
    ) -> Result<(), MojaveClientError> {
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
            method: "mojave_sendBroadcastBlock".to_string(),
            params: Some(vec![json!(params)]),
        };

        match self.send_request_race(request, sequencer_parsed_urls).await {
            Ok(RpcResponse::Success(result)) => {
                serde_json::from_value(result.result).map_err(MojaveClientError::from)
            }
            Ok(RpcResponse::Error(error_response)) => {
                Err(MojaveClientError::RpcError(error_response.error.message))
            }
            Err(error) => Err(error),
        }
    }

    pub async fn send_proof_input(
        &self,
        proof_input: &ProverData,
        prover_url: Url,
        sequencer_address: &str,
    ) -> Result<serde_json::Value, MojaveClientError> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: "mojave_sendProofInput".to_string(),
            params: Some(vec![json!(proof_input), json!(sequencer_address)]),
        };

        match self.send_request(request, vec![prover_url]).await {
            Ok(RpcResponse::Success(result)) => {
                serde_json::from_value(result.result).map_err(MojaveClientError::from)
            }
            Ok(RpcResponse::Error(error_response)) => {
                Err(MojaveClientError::RpcError(error_response.error.message))
            }
            Err(error) => Err(error),
        }
    }

    pub async fn get_job_id(
        &self,
        prover_url: Url,
    ) -> Result<serde_json::Value, MojaveClientError> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: "mojave_getJobId".to_string(),
            params: None,
        };

        match self.send_request(request, vec![prover_url]).await {
            Ok(RpcResponse::Success(result)) => {
                serde_json::from_value(result.result).map_err(MojaveClientError::from)
            }
            Ok(RpcResponse::Error(error_response)) => {
                Err(MojaveClientError::RpcError(error_response.error.message))
            }
            Err(error) => Err(error),
        }
    }

    pub async fn get_proof(
        &self,
        job_id: &str,
        prover_url: Url,
    ) -> Result<BatchProof, MojaveClientError> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: "mojave_getProof".to_string(),
            params: Some(vec![json!(job_id)]),
        };

        match self.send_request(request, vec![prover_url]).await {
            Ok(RpcResponse::Success(result)) => {
                serde_json::from_value(result.result).map_err(MojaveClientError::from)
            }
            Ok(RpcResponse::Error(error_response)) => {
                Err(MojaveClientError::RpcError(error_response.error.message))
            }
            Err(error) => Err(error),
        }
    }

    pub async fn send_batch_proof(
        &self,
        batch_proof: &BatchProof,
        sequencer_url: Url,
    ) -> Result<(), MojaveClientError> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: "mojave_sendBatchProof".to_string(),
            params: Some(vec![json!(batch_proof)]),
        };

        match self.send_request(request, vec![sequencer_url]).await {
            Ok(RpcResponse::Success(result)) => {
                serde_json::from_value(result.result).map_err(MojaveClientError::from)
            }
            Ok(RpcResponse::Error(error_response)) => {
                Err(MojaveClientError::RpcError(error_response.error.message))
            }
            Err(error) => Err(error),
        }
    }
}
