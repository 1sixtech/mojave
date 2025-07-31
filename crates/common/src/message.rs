use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::errors::MessageError;

pub struct Message;

impl Message {
    pub async fn send<T: Serialize>(
        stream: &mut (impl AsyncWriteExt + Unpin),
        data: &T,
    ) -> Result<(), MessageError> {
        let serialized = serde_json::to_vec(data)?;

        let length = serialized.len() as u32;
        stream.write_all(&length.to_be_bytes()).await?;

        stream.write_all(&serialized).await?;

        Ok(())
    }

    pub async fn receive<T: for<'de> Deserialize<'de>>(
        stream: &mut (impl AsyncReadExt + Unpin),
    ) -> Result<T, MessageError> {
        let mut length_bytes = [0u8; 4];
        stream.read_exact(&mut length_bytes).await?;

        let length = u32::from_be_bytes(length_bytes) as usize;
        let mut buffer = vec![0u8; length];
        stream.read_exact(&mut buffer).await?;

        let data = serde_json::from_slice(&buffer)?;
        Ok(data)
    }
}
