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

pub async fn receive<T>(stream: &mut TcpStream) -> Result<T, MessageError>
where
    T: DeserializeOwned,
{
    // TODO: check the packet length and return an error if it's too long.
    let length = stream.read_u32().await?;
    let mut buffer = Vec::with_capacity(length as usize);
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
}
