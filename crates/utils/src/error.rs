pub type NetworkResult<T> = core::result::Result<T, NetworkError>;

#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Custom(String),
}
