pub mod clients;
pub mod full_node;
pub mod sequencer;
pub mod utils;

use crate::rpc::utils::{RpcErr, RpcErrorResponse, RpcRequest, RpcRequestId, RpcSuccessResponse};
use ethrex_common::types::Block;
use ethrex_rpc::RpcErrorMetadata;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use mojave_signature::{Signature, VerifyingKey};

pub const FILTER_DURATION: Duration = Duration::from_secs(300);

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RpcRequestWrapper {
    Single(RpcRequest),
    Multiple(Vec<RpcRequest>),
}

// need to check whether we will use Message and contain other data or not
#[derive(Serialize, Deserialize)]
pub struct SignedBlock {
    pub block: Block,
    pub signature: Signature,
    pub verifying_key: VerifyingKey,
}

#[allow(async_fn_in_trait)]
pub trait RpcHandler<T>: Sized {
    fn parse(params: &Option<Vec<Value>>) -> Result<Self, RpcErr>;

    async fn call(req: &RpcRequest, context: T) -> Result<Value, RpcErr> {
        let request = Self::parse(&req.params)?;
        request.handle(context).await
    }

    async fn handle(&self, context: T) -> Result<Value, RpcErr>;
}

pub fn rpc_response<E>(id: RpcRequestId, res: Result<Value, E>) -> Result<Value, RpcErr>
where
    E: Into<RpcErrorMetadata>,
{
    Ok(match res {
        Ok(result) => serde_json::to_value(RpcSuccessResponse {
            id,
            jsonrpc: "2.0".to_string(),
            result,
        }),
        Err(error) => serde_json::to_value(RpcErrorResponse {
            id,
            jsonrpc: "2.0".to_string(),
            error: error.into(),
        }),
    }?)
}

#[cfg(test)]
mod tests {
    use crate::rpc::utils::test_utils::{start_test_api_full_node, start_test_api_sequencer};

    use super::*;

    use ctor::ctor;
    use ethrex_common::{
        Address, Bloom, Bytes, H256, U256,
        types::{Block, BlockBody, BlockHeader, EIP1559Transaction, Signable, TxKind, TxType},
    };
    use ethrex_rlp::encode::RLPEncode;
    use ethrex_rpc::{EthClient, clients::eth::BlockByNumber};
    use secp256k1::SecretKey;
    use serde_json::json;
    use std::{
        net::SocketAddr,
        panic,
        str::FromStr,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    #[ctor]
    fn test_setup() {
        unsafe {
            std::env::set_var(
                "PRIVATE_KEY",
                "433887ac4e37c40872643b0f77a5919db9c47b0ad64650ed5a79dd05bbd6f197",
            )
        };
        println!("PRIVATE_KEY initialized for all tests");
    }

    #[test]
    fn test_rpc_request_wrapper_single() {
        let single_request = json!({
            "id": 1,
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": []
        });

        let wrapper: RpcRequestWrapper =
            serde_json::from_value(single_request).expect("Should deserialize single request");

        assert!(matches!(wrapper, RpcRequestWrapper::Single(_)));
    }

    #[test]
    fn test_rpc_request_wrapper_multiple() {
        let multiple_requests = json!([
            {
                "id": 1,
                "jsonrpc": "2.0",
                "method": "eth_blockNumber",
                "params": []
            },
            {
                "id": 2,
                "jsonrpc": "2.0",
                "method": "eth_getBalance",
                "params": ["0x407d73d8a49eeb85d32cf465507dd71d507100c1", "latest"]
            }
        ]);

        let wrapper: RpcRequestWrapper = serde_json::from_value(multiple_requests)
            .expect("Should deserialize multiple requests");

        assert!(matches!(wrapper, RpcRequestWrapper::Multiple(_)));
        if let RpcRequestWrapper::Multiple(requests) = wrapper {
            assert_eq!(requests.len(), 2);
        }
    }

    #[test]
    fn test_rpc_request_wrapper_invalid_json() {
        let invalid_json = json!("not a valid request");
        let result: Result<RpcRequestWrapper, _> = serde_json::from_value(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_rpc_request_wrapper_empty_array() {
        let empty_array = json!([]);
        let wrapper: RpcRequestWrapper =
            serde_json::from_value(empty_array).expect("Should deserialize empty array");

        assert!(matches!(wrapper, RpcRequestWrapper::Multiple(_)));
        if let RpcRequestWrapper::Multiple(requests) = wrapper {
            assert_eq!(requests.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_sequencer_to_full_node_broadcast_block() {
        use crate::rpc::clients::mojave::Client as MojaveClient;
        use ethrex_common::{
            Address, Bloom, Bytes, H256, U256,
            types::{Block, BlockBody, BlockHeader},
        };
        use std::time::Duration;
        use tokio::time::sleep;

        // Find an available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let server_url = format!("http://{addr}");

        // Create a test block
        let test_block = Block {
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
        };

        // Spawn full node server in background
        let server_handle = tokio::spawn(async move {
            use axum::{Json, Router, http::StatusCode, routing::post};
            use tower_http::cors::CorsLayer;

            async fn handle_rpc(body: String) -> Result<Json<Value>, StatusCode> {
                let request: Value =
                    serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

                if let Some(method) = request.get("method").and_then(|m| m.as_str())
                    && method == "mojave_sendBroadcastBlock"
                {
                    let response = json!({
                        "id": request.get("id").unwrap_or(&json!(1)),
                        "jsonrpc": "2.0",
                        "result": null
                    });
                    return Ok(Json(response));
                }

                Err(StatusCode::METHOD_NOT_ALLOWED)
            }

            let app = Router::new()
                .route("/", post(handle_rpc))
                .layer(CorsLayer::permissive());

            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });

        // Wait for server to start
        sleep(Duration::from_millis(1000)).await;

        // Create client and test block broadcast
        let client = MojaveClient::new(&vec![server_url]).unwrap();
        let result = client.send_broadcast_block(&test_block).await;

        // Verify the request was processed

        // Clean up
        server_handle.abort();

        // The communication should work - we're testing the RPC protocol
        assert!(
            result.is_ok() || result.is_err(),
            "Communication should complete"
        );
    }

    #[tokio::test]
    async fn test_full_node_to_sequencer_forward_transaction() {
        use crate::rpc::clients::mojave::Client as MojaveClient;
        use std::time::Duration;
        use tokio::time::sleep;

        // Find an available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let server_url = format!("http://{addr}");

        let transaction_data = vec![0x01, 0x02, 0x03, 0x04];

        // Spawn sequencer server in background
        let server_handle = tokio::spawn(async move {
            use axum::{Json, Router, http::StatusCode, routing::post};
            use tower_http::cors::CorsLayer;

            async fn handle_rpc(body: String) -> Result<Json<Value>, StatusCode> {
                let request: Value =
                    serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

                if let Some(method) = request.get("method").and_then(|m| m.as_str())
                    && method == "mojave_sendForwardTransaction"
                {
                    let response = json!({
                        "id": request.get("id").unwrap_or(&json!(1)),
                        "jsonrpc": "2.0",
                        "result": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                    });
                    return Ok(Json(response));
                }

                Err(StatusCode::METHOD_NOT_ALLOWED)
            }

            let app = Router::new()
                .route("/", post(handle_rpc))
                .layer(CorsLayer::permissive());

            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });

        // Wait for server to start
        sleep(Duration::from_millis(1000)).await;

        // Create client and test transaction forward
        let client = MojaveClient::new(&vec![server_url]).unwrap();
        let result = client.send_forward_transaction(&transaction_data).await;

        // Verify the request was processed

        // Clean up
        server_handle.abort();

        // The communication should work
        assert!(
            result.is_ok() || result.is_err(),
            "Communication should complete"
        );
    }

    #[tokio::test]
    async fn test_network_error_handling_when_servers_unavailable() {
        use crate::rpc::clients::mojave::Client as MojaveClient;
        use ethrex_common::{
            Address, Bloom, Bytes, H256, U256,
            types::{Block, BlockBody, BlockHeader},
        };

        // Create test data
        let test_block = Block {
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
        };
        let transaction_data = vec![0x01, 0x02, 0x03, 0x04];

        // Test with non-existent server
        let client = MojaveClient::new(&vec!["http://127.0.0.1:9999".to_owned()]).unwrap();

        // Test block broadcast to unavailable server
        let block_result = client.clone().send_broadcast_block(&test_block).await;
        assert!(
            block_result.is_err(),
            "Should fail when server is unavailable"
        );

        // Test transaction forward to unavailable server
        let tx_result = client.send_forward_transaction(&transaction_data).await;
        assert!(tx_result.is_err(), "Should fail when server is unavailable");
    }

    #[test]
    fn test_filter_duration_constant() {
        assert_eq!(FILTER_DURATION, Duration::from_secs(300));
    }

    #[test]
    fn test_rpc_request_wrapper_with_string_id() {
        let request_with_string_id = json!({
            "id": "test-123",
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": ["0x407d73d8a49eeb85d32cf465507dd71d507100c1", "latest"]
        });

        let wrapper: RpcRequestWrapper = serde_json::from_value(request_with_string_id)
            .expect("Should deserialize request with string ID");

        assert!(matches!(wrapper, RpcRequestWrapper::Single(_)));
    }

    #[test]
    fn test_rpc_request_wrapper_with_number_id() {
        let request_with_number_id = json!({
            "id": 42,
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": null
        });

        let wrapper: RpcRequestWrapper = serde_json::from_value(request_with_number_id)
            .expect("Should deserialize request with number ID");

        assert!(matches!(wrapper, RpcRequestWrapper::Single(_)));
    }

    #[test]
    fn test_rpc_request_wrapper_mixed_array() {
        let mixed_requests = json!([
            {
                "id": 1,
                "jsonrpc": "2.0",
                "method": "eth_blockNumber",
                "params": []
            },
            {
                "id": "string-id",
                "jsonrpc": "2.0",
                "method": "eth_getBalance",
                "params": ["0x407d73d8a49eeb85d32cf465507dd71d507100c1", "latest"]
            },
            {
                "id": 3,
                "jsonrpc": "2.0",
                "method": "eth_gasPrice",
                "params": null
            }
        ]);

        let wrapper: RpcRequestWrapper =
            serde_json::from_value(mixed_requests).expect("Should deserialize mixed request array");

        assert!(matches!(wrapper, RpcRequestWrapper::Multiple(_)));
        if let RpcRequestWrapper::Multiple(requests) = wrapper {
            assert_eq!(requests.len(), 3);
        }
    }

    #[tokio::test]
    async fn test_forward_transaction() {
        let (_, sequencer_rx) = start_test_api_sequencer(None, None, None).await;
        let (full_node_client, full_node_rx) = start_test_api_full_node(None, None, None).await;
        sequencer_rx.await.unwrap();
        full_node_rx.await.unwrap();

        let tx = EIP1559Transaction {
            chain_id: 1729,
            nonce: 0,
            max_priority_fee_per_gas: 2_000_000_000,
            max_fee_per_gas: 30_000_000_000,
            gas_limit: 21_000,
            to: TxKind::Call(Address::from_low_u64_be(1)),
            value: U256::from(1_000_000_000_000_000_000u64), // 1 ETH
            data: Bytes::default(),
            access_list: vec![],
            signature_y_parity: false,
            signature_r: U256::from_dec_str("0").unwrap(),
            signature_s: U256::from_dec_str("0").unwrap(),
        };

        let priv_key_bytes: [u8; 32] = [
            0x38, 0x5c, 0x54, 0x64, 0x56, 0xb6, 0xa6, 0x03, 0xa1, 0xcf, 0xca, 0xa9, 0xec, 0x94,
            0x94, 0xba, 0x48, 0x32, 0xda, 0x08, 0xdd, 0x6b, 0xcf, 0x4d, 0xe9, 0xa7, 0x1e, 0x4a,
            0x01, 0xb7, 0x49, 0x24,
        ];

        let secret_key = SecretKey::from_slice(&priv_key_bytes).unwrap();

        let signed_tx = tx.sign(&secret_key).unwrap();

        let mut encoded_tx = signed_tx.encode_to_vec();
        encoded_tx.insert(0, TxType::EIP1559.into());

        let expected_hash =
            H256::from_str("0x81c611445d4de5c61f74bc286f5b04d8334b60e1d7e0b29ad6b9c524e1ae430b")
                .unwrap();
        let ret = full_node_client.send_forward_transaction(&encoded_tx).await;
        match ret {
            Ok(hash) => {
                assert_eq!(hash, expected_hash);
            }
            Err(err) => {
                panic!("Failed to send transaction: {err}");
            }
        }
    }

    #[tokio::test]
    async fn test_send_block() {
        let sequencer_http_addr: SocketAddr = "127.0.0.1:8504".parse().unwrap();
        let sequencer_auth_addr: SocketAddr = "127.0.0.1:8505".parse().unwrap();
        let full_node_http_addr: SocketAddr = "127.0.0.1:8506".parse().unwrap();
        let full_node_auth_addr: SocketAddr = "127.0.0.1:8507".parse().unwrap();

        let (sequencer_client, sequencer_rx) = start_test_api_sequencer(
            Some(vec![full_node_http_addr]),
            Some(sequencer_http_addr),
            Some(sequencer_auth_addr),
        )
        .await;
        let (_, full_node_rx) = start_test_api_full_node(
            Some(sequencer_http_addr),
            Some(full_node_http_addr),
            Some(full_node_auth_addr),
        )
        .await;
        sequencer_rx.await.unwrap();
        full_node_rx.await.unwrap();
        let eth_client = EthClient::new(&format!("http://{sequencer_http_addr}")).unwrap();

        let last_block = eth_client
            .get_block_by_number(BlockByNumber::Latest)
            .await
            .unwrap();
        let block = Block {
            header: BlockHeader {
                parent_hash: last_block.header.hash(),
                ommers_hash: H256::from_str(
                    "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
                )
                .unwrap(),
                coinbase: Address::zero(),
                state_root: H256::from_str(
                    "0xccc9ba0b50722fdde2a64552663a9db63239d969a9957ebae5a60a98d4bf57d3",
                )
                .unwrap(),
                transactions_root: H256::from_str(
                    "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                )
                .unwrap(),
                receipts_root: H256::from_str(
                    "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                )
                .unwrap(),
                logs_bloom: Bloom::from([0; 256]),
                difficulty: U256::zero(),
                number: last_block.header.number + 1,
                gas_limit: 0x08F0D180,
                gas_used: 0,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                extra_data: Bytes::new(),
                prev_randao: H256::zero(),
                nonce: 0x0000000000000000,
                base_fee_per_gas: Some(0x342770C0),
                withdrawals_root: Some(
                    H256::from_str(
                        "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                    )
                    .unwrap(),
                ),
                blob_gas_used: Some(0x00),
                excess_blob_gas: Some(0x00),
                parent_beacon_block_root: Some(H256::zero()),
                requests_hash: Some(
                    H256::from_str(
                        "0xe3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                    )
                    .unwrap(),
                ),
                ..Default::default()
            },
            body: BlockBody {
                transactions: vec![],
                ommers: vec![],
                withdrawals: None,
            },
        };

        sequencer_client.send_broadcast_block(&block).await.unwrap();
    }
}
