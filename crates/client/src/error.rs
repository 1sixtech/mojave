pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error: {0}")]
    Custom(String),
    #[error("Invalid max attempts: {0}")]
    InvalidMaxAttempts(u64),
    #[error("No RPC URLs configured")]
    NoRPCUrlsConfigured,
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Retry failed after {0} attempts")]
    RetryFailed(u64),
    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Signature error: {0}")]
    SignatureError(#[from] mojave_signature::error::Error),
    #[error("Connection timed out")]
    TimeOut,
}
