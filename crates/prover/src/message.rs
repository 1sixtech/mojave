use crate::types::*;
use ethrex_l2_common::prover::BatchProof;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum Request {
    Proof(ProverData),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Response {
    Proof(BatchProof),
    Error(String),
}

const MAX_MESSAGE_SIZE: u32 = 10 * 1024 * 1024; // 10MB

pub async fn receive<T>(stream: &mut TcpStream) -> Result<T, MessageError>
where
    T: DeserializeOwned,
{
    let length = stream.read_u32().await?;
    if length > MAX_MESSAGE_SIZE {
        return Err(MessageError::MessageTooLarge(MAX_MESSAGE_SIZE, length));
    }
    let mut buffer = vec![0; length as usize];
    stream.read_exact(&mut buffer).await?;
    serde_json::from_slice(&buffer).map_err(MessageError::Deserialize)
}

pub async fn send<T>(stream: &mut TcpStream, data: T) -> Result<(), MessageError>
where
    T: Serialize,
{
    let serialized = serde_json::to_vec(&data).map_err(MessageError::Serialize)?;
    let length = serialized.len() as u32;
    stream.write_u32(length).await?;
    stream.write_all(&serialized).await?;
    stream.flush().await?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum MessageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Deserialization error: {0}")]
    Deserialize(serde_json::Error),
    #[error("Serialization error: {0}")]
    Serialize(serde_json::Error),
    #[error("Message is too large. max: {0}, got: {1}")]
    MessageTooLarge(u32, u32),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    // TODO: add test case when we can have program input mock data and proof result of it

    #[tokio::test]
    async fn send_receive_over_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut stream, _addr) = listener.accept().await.unwrap();
            send(&mut stream, "Response::Proof(dummy_proof)")
                .await
                .unwrap();
        });

        let mut stream = TcpStream::connect(addr).await.unwrap();
        let _response: String = receive(&mut stream).await.unwrap();

        server.await.unwrap();
    }

    #[tokio::test]
    async fn send_receive_error_over_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut stream, _addr) = listener.accept().await.unwrap();
            send(
                &mut stream,
                Response::Error("Error while generate proof".to_string()),
            )
            .await
            .unwrap();
        });

        let mut stream = TcpStream::connect(addr).await.unwrap();
        let _response: Response = receive(&mut stream).await.unwrap();

        server.await.unwrap();
    }
}
