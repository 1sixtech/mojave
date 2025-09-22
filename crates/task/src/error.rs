#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to send the request: {0}")]
    Send(String),
    #[error("Failed to receive a response: {0}")]
    Receive(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("Task error: {0}")]
    Task(Box<dyn std::error::Error>),
}
