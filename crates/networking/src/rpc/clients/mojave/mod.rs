use ed25519_dalek::{Signature, SigningKey};
use ethrex_common::{H256, types::Block};
use futures::{
    FutureExt,
    future::{Fuse, select_ok},
};
use k256::ecdsa::signature::SignerMut;
use reqwest::Url;
use serde::Deserialize;
use serde_json::json;
use std::{env, pin::Pin, sync::Arc};

use crate::rpc::{
    SignedBlock,
    clients::mojave::errors::{ForwardTransactionError, MojaveClientError},
    utils::{RpcErrorResponse, RpcRequest, RpcRequestId, RpcSuccessResponse},
};

pub mod errors;

#[derive(Debug)]
struct ClientInner {
    client: reqwest::Client,
    pub urls: Vec<Url>,
}

#[derive(Clone, Debug)]
pub struct Client {
    inner: Arc<ClientInner>,
    signing_key: SigningKey,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum RpcResponse {
    Success(RpcSuccessResponse),
    Error(RpcErrorResponse),
}

impl Client {
    pub fn new(urls: Vec<&str>) -> Result<Self, MojaveClientError> {
        tracing::info!(urls=%urls.join(", "), "Creating new Mojave client");
        let urls = urls
            .iter()
            .map(|url| {
                Url::parse(url).map_err(|_| {
                    MojaveClientError::ParseUrlError("Failed to parse urls".to_string())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let secret = env::var("PRIVATE_KEY").map_err(|_| MojaveClientError::MissingSigningKey)?;

        let private_key_bytes = hex::decode(secret).expect("Failed to decode private key from hex");
        let private_key_array: [u8; 32] = private_key_bytes
            .try_into()
            .expect("invalid length for private key");

        let signing_key = SigningKey::from_bytes(&private_key_array);

        Ok(Self {
            inner: Arc::new(ClientInner {
                client: reqwest::Client::new(),
                urls,
            }),
            signing_key,
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
    /// Returns the response from the last successful request, or the last error if all requests fail.
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
            .post(url.as_str())
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

    pub async fn send_forward_transaction(&self, data: &[u8]) -> Result<H256, MojaveClientError> {
        let request = RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: "mojave_sendForwardTransaction".to_string(),
            params: Some(vec![json!("0x".to_string() + &hex::encode(data))]),
        };
        match self.send_request(request).await {
            Ok(RpcResponse::Success(result)) => serde_json::from_value(result.result)
                .map_err(ForwardTransactionError::SerdeJSONError)
                .map_err(MojaveClientError::from),
            Ok(RpcResponse::Error(error_response)) => {
                Err(ForwardTransactionError::RPCError(error_response.error.message).into())
            }
            Err(error) => Err(error),
        }
    }

    pub async fn send_broadcast_block(&self, block: &Block) -> Result<(), MojaveClientError> {
        let hash = block.hash();
        let mut signing_key_clone = self.signing_key.clone();
        let signature: Signature = signing_key_clone.sign(hash.as_bytes());

        let params = SignedBlock {
            block: block.clone(),
            signature: signature.to_bytes(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::clients::mojave::errors::MojaveClientError;
    use ethrex_common::{
        Address, Bloom, Bytes, H256, U256,
        types::{Block, BlockBody, BlockHeader},
    };
    use mockito::mock;
    use serde_json::json;

    fn create_test_block() -> Block {
        Block {
            header: BlockHeader {
                parent_hash: H256::zero(),
                ommers_hash: H256::zero(),
                coinbase: Address::zero(),
                state_root: H256::zero(),
                transactions_root: H256::zero(),
                receipts_root: H256::zero(),
                logs_bloom: Bloom::default(),
                difficulty: U256::zero(),
                number: 1u64,
                gas_limit: 21000u64,
                gas_used: 0u64,
                timestamp: 0u64,
                extra_data: Bytes::new(),
                prev_randao: H256::zero(),
                nonce: 0u64,
                base_fee_per_gas: Some(0u64),
                withdrawals_root: None,
                blob_gas_used: None,
                excess_blob_gas: None,
                parent_beacon_block_root: None,
                requests_hash: None,
                ..Default::default()
            },
            body: BlockBody {
                transactions: vec![],
                ommers: vec![],
                withdrawals: None,
            },
        }
    }

    #[test]
    fn test_client_new_success() {
        let urls = vec!["http://localhost:8545", "http://localhost:8546"];
        let client = Client::new(urls);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.inner.urls.len(), 2);
    }

    #[test]
    fn test_client_new_invalid_url() {
        let urls = vec!["invalid-url"];
        let result = Client::new(urls);
        assert!(result.is_err());
        assert!(matches!(result, Err(MojaveClientError::ParseUrlError(_))));
    }

    #[test]
    fn test_client_new_mixed_urls() {
        let urls = vec!["http://localhost:8545", "invalid-url"];
        let result = Client::new(urls);
        assert!(result.is_err());
    }

    #[test]
    fn test_client_new_empty_urls() {
        let urls: Vec<&str> = vec![];
        let client = Client::new(urls);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.inner.urls.len(), 0);
    }

    #[tokio::test]
    async fn test_send_forward_transaction_success() {
        let _mock = mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "id": 1,
                    "jsonrpc": "2.0",
                    "result": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                })
                .to_string(),
            )
            .create();

        let client = Client::new(vec![&mockito::server_url()]).unwrap();
        let data = vec![0x12, 0x34];
        let result = client.send_forward_transaction(&data).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_forward_transaction_rpc_error() {
        let _mock = mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "id": 1,
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32602,
                        "message": "Invalid params"
                    }
                })
                .to_string(),
            )
            .create();

        let client = Client::new(vec![&mockito::server_url()]).unwrap();
        let data = vec![0x12, 0x34];
        let result = client.send_forward_transaction(&data).await;

        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(MojaveClientError::ForwardTransactionError(_))
        ));
    }

    #[tokio::test]
    async fn test_send_forward_transaction_network_error() {
        let client = Client::new(vec!["http://nonexistent:9999"]).unwrap();
        let data = vec![0x12, 0x34];
        let result = client.send_forward_transaction(&data).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(MojaveClientError::Custom(_))));
    }

    #[tokio::test]
    async fn test_send_broadcast_block_success() {
        let _block = create_test_block();

        let _mock = mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "id": 1,
                    "jsonrpc": "2.0",
                    "result": null
                })
                .to_string(),
            )
            .create();

        let client = Client::new(vec![&mockito::server_url()]).unwrap();
        let block = create_test_block();
        let result = client.send_broadcast_block(&block).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_broadcast_block_rpc_error() {
        let _mock = mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "id": 1,
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": "Internal error"
                    }
                })
                .to_string(),
            )
            .create();

        let client = Client::new(vec![&mockito::server_url()]).unwrap();
        let block = create_test_block();
        let result = client.send_broadcast_block(&block).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(MojaveClientError::RpcError(_))));
    }

    #[tokio::test]
    async fn test_send_broadcast_block_network_error() {
        let client = Client::new(vec!["http://nonexistent:9999"]).unwrap();
        let block = create_test_block();
        let result = client.send_broadcast_block(&block).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(MojaveClientError::Custom(_))));
    }

    #[test]
    fn test_rpc_response_deserialization_success() {
        let json_str = r#"{"id":1,"jsonrpc":"2.0","result":"success"}"#;
        let response: RpcResponse = serde_json::from_str(json_str).unwrap();
        assert!(matches!(response, RpcResponse::Success(_)));
    }

    #[test]
    fn test_rpc_response_deserialization_error() {
        let json_str =
            r#"{"id":1,"jsonrpc":"2.0","error":{"code":-32602,"message":"Invalid params"}}"#;
        let response: RpcResponse = serde_json::from_str(json_str).unwrap();
        assert!(matches!(response, RpcResponse::Error(_)));
    }

    #[test]
    fn test_client_arc_cloning() {
        let urls = vec!["http://localhost:8545"];
        let client = Client::new(urls).unwrap();

        // Clone the client multiple times
        let client_clone1 = client.clone();
        let client_clone2 = client.clone();

        // Verify that all instances point to the same URLs
        assert_eq!(client.inner.urls.len(), 1);
        assert_eq!(client_clone1.inner.urls.len(), 1);
        assert_eq!(client_clone2.inner.urls.len(), 1);

        // Verify URL content is the same
        assert_eq!(client.inner.urls[0].as_str(), "http://localhost:8545/");
        assert_eq!(
            client_clone1.inner.urls[0].as_str(),
            "http://localhost:8545/"
        );
        assert_eq!(
            client_clone2.inner.urls[0].as_str(),
            "http://localhost:8545/"
        );
    }
}
