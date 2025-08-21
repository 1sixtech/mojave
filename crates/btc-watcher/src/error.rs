use bitcoin::Block;

#[derive(Debug, thiserror::Error)]
pub enum BlockWatcherError {
    #[error("ZMQ error: {0}")]
    ZmqError(#[from] zeromq::ZmqError),
    #[error("Tokio send error: {0}")]
    TokioSendError(#[from] tokio::sync::broadcast::error::SendError<Block>),
    #[error("Bitcoin deserialization error: {0}")]
    DeserializationError(#[from] bitcoin::consensus::encode::Error),
    #[error("Join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}
