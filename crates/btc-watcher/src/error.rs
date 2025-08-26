#[derive(Debug, thiserror::Error)]
pub enum Error<T>
where
    T: core::fmt::Debug,
{
    #[error("ZMQ error: {0}")]
    ZmqError(#[from] zeromq::ZmqError),
    #[error("Tokio send error: {0}")]
    TokioSendError(#[from] tokio::sync::broadcast::error::SendError<T>),
    #[error("Bitcoin deserialization error: {0}")]
    DeserializationError(#[from] bitcoin::consensus::encode::Error),
    #[error("Join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}
