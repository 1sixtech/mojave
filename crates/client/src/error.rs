#[derive(Debug, thiserror::Error)]
pub enum MojaveClientError {
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("RPC error: {0}")]
    RpcError(String),
    #[error("Error: {0}")]
    Custom(String),
    #[error("Signature error: {0}")]
    SignatureError(#[from] mojave_signature::SignatureError),
    #[error("No RPC URLs configured")]
    NoRPCUrlsConfigured,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Message error: {0}")]
    Message(#[from] mojave_prover::MessageError),
    #[error("Internal server error: {0}")]
    Internal(String),
    #[error("Unexpected error: {0}")]
    Unexpected(String),
    #[error("Connection timed out")]
    TimeOut,
    #[error("Retry failed after {0} attempts")]
    RetryFailed(u64),
}
