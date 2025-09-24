use crate::{constants::DEFAULT_TASK_CAPACITY, handle::TaskHandle, runner::TaskRunner};
use tokio::sync::{mpsc, oneshot};

#[trait_variant::make(Send)]
pub trait Task: Sized + 'static {
    type Request: Send + 'static;
    type Response: std::fmt::Debug + Send + 'static;
    type Error: std::error::Error + Send + 'static;

    async fn handle_request(&self, request: Self::Request) -> Result<Self::Response, Self::Error>;

    async fn on_shutdown(&self) -> Result<(), Self::Error>;

    fn spawn_with_capacity(self, capacity: usize) -> TaskHandle<Self> {
        let (request_sender, request_receiver) = mpsc::channel::<(
            Self::Request,
            oneshot::Sender<Result<Self::Response, Self::Error>>,
        )>(capacity);
        let (shutdown_sender, shutdown_receiver) =
            mpsc::channel::<oneshot::Sender<Result<(), Self::Error>>>(capacity);

        let mut runner = TaskRunner::new(request_receiver, shutdown_receiver, self);
        tokio::spawn(async move {
            runner.listen().await;
        });
        TaskHandle::new(request_sender, shutdown_sender)
    }

    fn spawn(self) -> TaskHandle<Self> {
        self.spawn_with_capacity(DEFAULT_TASK_CAPACITY)
    }
}
