use crate::types::{Publisher, Result};

pub struct Dummy;

impl Dummy {
    pub async fn new() -> Result<Self> {
        Ok(Self {})
    }
}

#[async_trait::async_trait]
impl Publisher for Dummy {
    async fn publish(&self, _msg: bytes::Bytes) -> Result<()> {
        Ok(())
    }
}
