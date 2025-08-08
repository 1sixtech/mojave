#[cfg(feature = "client")]
use mojave_prover::ProverClient;
use mojave_prover::ProverData;
#[cfg(feature = "server")]
use mojave_prover::ProverServer;
use tokio::net::TcpListener;
use zkvm_interface::io::ProgramInput;

#[cfg(feature = "server")]
async fn start_server() -> String {
    let temp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = temp.local_addr().unwrap();
    drop(temp);

    let mut server = ProverServer::new(false, &addr.to_string()).await;
    tokio::spawn(async move {
        server.start().await;
    });

    addr.to_string()
}

#[cfg(feature = "client")]
async fn create_mock_client() -> ProverClient {
    let server_addr = start_server().await;

    ProverClient::new(&server_addr, 10)
}

#[cfg(feature = "client")]
fn create_mock_prover_data() -> ProverData {
    ProverData {
        batch_number: 1,
        input: ProgramInput::default(),
    }
}

#[cfg(all(feature = "client", feature = "server"))]
#[tokio::test]
async fn test_client_server_communication() {
    let mut client = create_mock_client().await;

    match client.get_proof(create_mock_prover_data()).await {
        Ok(data) => println!("Success! proof is: {:?}", data),
        Err(error) => println!("Error! message is: {:?}", error),
    }
}

#[cfg(feature = "client")]
#[tokio::test]
async fn test_client_connection_refused() {
    let mut client = ProverClient::new("127.0.0.1:1", 10);
    match client.get_proof(create_mock_prover_data()).await {
        Ok(_) => panic!("Should receive error"),
        Err(error) => println!("Error! message is: {:?}", error),
    }
}

#[cfg(feature = "client")]
#[tokio::test]
async fn test_client_timeout() {
    let mut client = ProverClient::new("192.0.2.1:12345", 2);
    match client.get_proof(create_mock_prover_data()).await {
        Ok(_) => panic!("Should receive timeout error"),
        Err(error) => println!("Error! message is: {:?}", error),
    }
}
