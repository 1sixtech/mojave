#[cfg(test)]
mod tests {
    use ctor::ctor;
    use ethrex_common::{
        Address, Bytes, H256, U256,
        types::{EIP1559Transaction, TxKind, TxType},
    };
    use ethrex_l2_rpc::signer::{LocalSigner, Signable, Signer};
    use ethrex_rlp::encode::RLPEncode;
    use ethrex_rpc::EthClient;
    use mojave_tests::{start_test_api_node, start_test_api_sequencer};
    use once_cell::sync::OnceCell;
    use secp256k1::SecretKey;
    use serde_json::{Value, json};
    use std::str::FromStr;
    use tokio::time::{Duration, sleep};
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
            use axum::{Json, Router, http::StatusCode, routing::post};
            use tower_http::cors::CorsLayer;

            async fn handle_rpc(body: String) -> Result<Json<Value>, StatusCode> {
                let request: Value =
                    serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

                if let Some(method) = request.get("method").and_then(|m| m.as_str())
                    && method == "eth_sendRawTransaction"
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
        sleep(Duration::from_millis(100)).await;

        // create mojave client and test transaction forward
        let client = EthClient::new(&server_url).unwrap();
        let result = client.send_raw_transaction(&transaction_data).await;

        server_handle.abort();

        // assert the response
        assert!(result.is_ok(), "Communication should complete");
    }

    #[tokio::test]
    async fn test_forward_transaction() {
        let (_, sequencer_rx) = start_test_api_sequencer(None, None).await;
        let (full_node_client, full_node_rx) = start_test_api_node(None, None, None).await;
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
            inner_hash: OnceCell::new(),
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
}
