use crate::{MojaveClientError, types::SignedBlock};
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
use reqwest::Url;
use serde_json::json;
use std::{env, pin::Pin, str::FromStr, sync::Arc};

#[derive(Clone, Debug)]
pub struct MojaveClient {
    inner: Arc<MojaveClientInner>,
}

#[derive(Debug)]
struct MojaveClientInner {
    client: reqwest::Client,
    urls: Vec<Url>,
    signing_key: SigningKey,
}

impl MojaveClient {
    pub fn new(full_node_addresses: &[String]) -> Result<Self, MojaveClientError> {
        let private_key = env::var("PRIVATE_KEY")
            .map_err(|error| MojaveClientError::Custom(format!("Private key error: {error}")))?;
        let urls = full_node_addresses
            .iter()
            .map(|url| {
                Url::parse(url).map_err(|error| MojaveClientError::ParseUrlError(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let signing_key = SigningKey::from_str(&private_key)?;
        Ok(Self {
            inner: Arc::new(MojaveClientInner {
                client: reqwest::Client::new(),
                urls,
                signing_key,
            }),
        })
    }

    /// Sends multiple RPC requests to a list of urls and returns
    /// the first response without waiting for others to finish.
    async fn send_request_race(
        &self,
        request: RpcRequest,
    ) -> Result<RpcResponse, MojaveClientError> {
        let requests: Vec<Pin<Box<Fuse<_>>>> = self
            .inner
            .urls
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
    async fn send_request(&self, request: RpcRequest) -> Result<RpcResponse, MojaveClientError> {
        let mut response = Err(MojaveClientError::Custom(
            "All rpc calls failed".to_string(),
        ));

        for url in self.inner.urls.iter() {
            let maybe_response = self.send_request_to_url(url, &request).await;
            if maybe_response.is_ok() {
                response = maybe_response;
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

    pub async fn send_broadcast_block(&self, block: &Block) -> Result<(), MojaveClientError> {
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

        match self.send_request_race(request).await {
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
