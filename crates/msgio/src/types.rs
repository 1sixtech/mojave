use async_trait::async_trait;
use bytes::Bytes;

use crate::error::Result;

#[async_trait]
pub trait Publisher: Send + Sync + 'static {
    async fn publish(&self, msg: Bytes) -> Result<()>;
}
