#[derive(Debug, thiserror::Error)]
pub enum MojaveClientError {
    #[error("Failed to serialize the request body: {0}")]
    SerializeRequest(serde_json::Error),
    #[error("Failed to send a request: {0}")]
    SendRequest(reqwest::Error),
    #[error("Failed to deserialize the response: {0}")]
    DeserializeResponse(reqwest::Error),
    #[error("Failed to deserialize the response result: {0}")]
    DeserializeResponseResult(serde_json::Error),
    #[error("RPC error: {0}")]
    RpcError(String),
    #[error("Error: {0}")]
    Custom(String),
    #[error("Signature error: {0}")]
    SignatureError(#[from] mojave_signature::SignatureError),
    #[error("No RPC URLs configured")]
    NoRPCUrlsConfigured,
}
