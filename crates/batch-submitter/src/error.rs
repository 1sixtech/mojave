pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Batch notifier error: {0}")]
    BatchNotifierError(#[from] tokio::sync::mpsc::error::TrySendError<u64>),
}
