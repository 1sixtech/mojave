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
    #[error("Connection timed out")]
    TimeOut,
    #[error("Retry failed after {0} attempts")]
    RetryFailed(u64),
    #[error("Missing full node URLs")]
    MissingFullNodeUrls,
    #[error("Missing prover URL")]
    MissingProverUrl,
    #[error("Missing sequencer URL")]
    MissingSequencerUrl,
    #[error("Missing max attempts")]
    MissingMaxAttempts,
    #[error("Missing timeout")]
    MissingTimeout,
    #[error("Missing private key")]
    MissingPrivateKey,
}
