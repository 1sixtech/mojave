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
}
