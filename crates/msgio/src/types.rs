use async_trait::async_trait;
use bytes::Bytes;

use crate::error::Error;
pub type Result<T> = core::result::Result<T, Error>;

#[async_trait]
pub trait Publisher: Send + Sync + 'static {
    async fn publish(&self, msg: Bytes) -> Result<()>;
}
