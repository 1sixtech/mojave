#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to send the request: {0}")]
    Send(String),
    #[error("Failed to receive a response: {0}")]
    Receive(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("Task handle for {0} dropped..")]
    TaskHandleDropped(&'static str),
}
