#[cfg(test)]
mod tests {
    use ctor::ctor;
    use ethrex_common::{
        types::{Block, BlockBody, BlockHeader, EIP1559Transaction, TxKind, TxType},
        Address, Bloom, Bytes, H256, U256,
    };
    use ethrex_l2_rpc::signer::{LocalSigner, Signable, Signer};
    use ethrex_rlp::encode::RLPEncode;
    use ethrex_rpc::{
        types::block_identifier::{BlockIdentifier, BlockTag},
        EthClient,
    };
    use mojave_client::MojaveClient;
    use mojave_tests::{start_test_api_full_node, start_test_api_sequencer};
    use reqwest::Url;
    use secp256k1::SecretKey;
    use serde_json::{json, Value};
    use std::{
        net::SocketAddr,
        str::FromStr,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tokio::time::{sleep, Duration};
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

    #[tokio::test]
    async fn test_sequencer_to_full_node_broadcast_block() {
        // create a test block
        let test_block = create_test_block();

        // Find an available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let server_url = format!("http://{addr}");

        // spawn full node server
        let server_handle = tokio::spawn(async move {
            use axum::{http::StatusCode, routing::post, Json, Router};
            use tower_http::cors::CorsLayer;

            async fn handle_rpc(body: String) -> Result<Json<Value>, StatusCode> {
                let request: Value =
                    serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

                if let Some(method) = request.get("method").and_then(|m| m.as_str()) {
                    if method == "mojave_sendBroadcastBlock" {
                        let response = json!({
                            "id": request.get("id").unwrap_or(&json!(1)),
                            "jsonrpc": "2.0",
                            "result": null
                        });
                        return Ok(Json(response));
                    }
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
        sleep(Duration::from_millis(100)).await;

        // create mojave client and test block broadcast
        let private_key = std::env::var("PRIVATE_KEY").unwrap();
        let client = MojaveClient::new(&private_key).unwrap();
        let result = client
            .send_broadcast_block(&test_block, &[Url::parse(&server_url).unwrap()])
            .await;

        server_handle.abort();

        // assert the response
        assert!(result.is_ok(), "Communication should complete");
    }

    #[tokio::test]
    async fn test_full_node_to_sequencer_forward_transaction() {
        // create a test transaction
        let transaction_data = vec![0x01, 0x02, 0x03, 0x04];

        // Find an available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let server_url = format!("http://{addr}");

        // spawn sequencer server
        let server_handle = tokio::spawn(async move {
            use axum::{http::StatusCode, routing::post, Json, Router};
            use tower_http::cors::CorsLayer;

            async fn handle_rpc(body: String) -> Result<Json<Value>, StatusCode> {
                let request: Value =
                    serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

                if let Some(method) = request.get("method").and_then(|m| m.as_str()) {
                    if method == "eth_sendRawTransaction" {
                        let response = json!({
                            "id": request.get("id").unwrap_or(&json!(1)),
                            "jsonrpc": "2.0",
                            "result": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                        });
                        return Ok(Json(response));
                    }
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
        sleep(Duration::from_millis(100)).await;

        // create mojave client and test transaction forward
        let client = EthClient::new(&server_url).unwrap();
        let result = client.send_raw_transaction(&transaction_data).await;

        server_handle.abort();

        // assert the response
        assert!(result.is_ok(), "Communication should complete");
    }

    #[tokio::test]
    async fn test_network_error_handling_when_servers_unavailable() {
        // create a test block
        let test_block = create_test_block();

        // Find an available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let server_url = format!("http://{addr}");

        // create mojave client and test block broadcast
        let private_key = std::env::var("PRIVATE_KEY").unwrap();
        let client = MojaveClient::new(&private_key).unwrap();
        let result = client
            .send_broadcast_block(&test_block, &[Url::parse(&server_url).unwrap()])
            .await;

        // assert the response
        assert!(result.is_err(), "Should fail when server is unavailable");
    }

    #[tokio::test]
    async fn test_forward_transaction() {
        let (_, sequencer_rx) = start_test_api_sequencer(None, None, None).await;
        let (full_node_client, full_node_rx) = start_test_api_full_node(None, None, None).await;
        sequencer_rx.await.unwrap();
        full_node_rx.await.unwrap();

        // send a transaction to the full node
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

        let signer = Signer::Local(LocalSigner::new(secret_key));

        let signed_tx = tx.sign(&signer).await.unwrap();

        let mut encoded_tx = signed_tx.encode_to_vec();
        encoded_tx.insert(0, TxType::EIP1559.into());

        let expected_hash =
            H256::from_str("0x81c611445d4de5c61f74bc286f5b04d8334b60e1d7e0b29ad6b9c524e1ae430b")
                .unwrap();

        let ret = full_node_client
            .send_raw_transaction(&encoded_tx)
            .await
            .unwrap();

        assert_eq!(ret, expected_hash);
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
            .get_block_by_number(BlockIdentifier::Tag(BlockTag::Latest))
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

        let full_node_urls = vec![Url::parse(&format!("http://{full_node_http_addr}")).unwrap()];

        sequencer_client
            .send_broadcast_block(&block, &full_node_urls)
            .await
            .unwrap();
    }
}
