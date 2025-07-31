#[derive(Debug, thiserror::Error)]
pub enum MessageError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Write error: {0}")]
    Write(#[from] std::io::Error),
    #[error("Read error: {0}")]
    Read(std::io::Error),
    #[error("Deserialization error: {0}")]
    Deserialization(serde_json::Error),
    #[error("Message too large: {0} bytes")]
    MessageTooLarge(usize),
}