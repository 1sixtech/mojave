use ethrex_common::{H256, Address, Bloom, U256, Bytes};
use ethrex_common::types::{Block, BlockHeader, BlockBody};
use mojave_client::MojaveClient;
use ctor::ctor;
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};
use ethrex_rpc::EthClient;

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

    test_block
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
        use axum::{Json, Router, http::StatusCode, routing::post};
        use tower_http::cors::CorsLayer;

        async fn handle_rpc(body: String) -> Result<Json<Value>, StatusCode> {
            let request: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

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
    let client = MojaveClient::new(std::slice::from_ref(&server_url), &private_key).unwrap();
    let result = client.send_broadcast_block(&test_block).await;


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
        use axum::{Json, Router, http::StatusCode, routing::post};
        use tower_http::cors::CorsLayer;

        async fn handle_rpc(body: String) -> Result<Json<Value>, StatusCode> {
            let request: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

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
    let client = MojaveClient::new(std::slice::from_ref(&server_url), &private_key).unwrap();
    let result = client.send_broadcast_block(&test_block).await;

    // assert the response 
    assert!(result.is_err(), "Should fail when server is unavailable");
}

#[tokio::test]
async fn test_forward_transaction() {

}

#[tokio::test]
async fn test_send_block() {
    
}

