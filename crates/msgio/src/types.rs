use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[async_trait]
pub trait Publisher: Send + Sync + 'static {
    async fn publish(&self, msg: Bytes) -> Result<()>;
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MessageKind {
    ProofResponse,
    BatchSubmit,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageHeader {
    pub version: u8,
    pub kind: MessageKind,
    pub message_id: String,
    pub seq: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message<T> {
    pub header: MessageHeader,
    pub body: T,
}
