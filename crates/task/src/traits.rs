use std::time::Duration;

use crate::{constants::DEFAULT_TASK_CAPACITY, handle::TaskHandle, task_runner::TaskRunner};
use tokio::{
    sync::{mpsc, oneshot},
    time::{MissedTickBehavior, interval},
};

#[trait_variant::make(Send)]
pub trait Task: Sized + 'static {
    type Request: Send + 'static;
    type Response: std::fmt::Debug + Send + 'static;
    type Error: std::error::Error + Send + Sync + 'static;

    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    // Default no-op startup hook
    async fn on_start(&mut self) -> Result<(), Self::Error> {
        std::future::ready(Ok(()))
    }

    async fn handle_request(
        &mut self,
        request: Self::Request,
    ) -> Result<Self::Response, Self::Error>;

    fn on_request_started(&mut self, _req: &Self::Request) {}
    fn on_request_finished(&mut self, _res: &Result<Self::Response, Self::Error>) {}

    // Default no-op shutdown hook
    async fn on_shutdown(&mut self) -> Result<(), Self::Error> {
        std::future::ready(Ok(()))
    }

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

    /// Spawn the task and also start a periodic job that submits a request every `every`.
    ///
    /// The `make_request` closure is called on each tick to build the request,
    /// so `Self::Request` does **not** need to be `Clone`.
    ///
    /// The loop stops automatically when the task shuts down or the handle is dropped.
    fn spawn_with_capacity_periodic<F>(
        self,
        capacity: usize,
        every: Duration,
        mut make_request: F,
    ) -> TaskHandle<Self>
    where
        F: FnMut() -> Self::Request + Send + 'static,
    {
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

        let periodic_sender = request_sender.clone();
        tokio::spawn(async move {
            let mut tick = interval(every);
            tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                let req = make_request();
                let (tx, rx) = oneshot::channel();
                if periodic_sender.send((req, tx)).await.is_err() {
                    break;
                }
                let _ = rx.await;
            }
        });

        TaskHandle::new(request_sender, shutdown_sender)
    }

    fn spawn_periodic<F>(self, every: Duration, make_request: F) -> TaskHandle<Self>
    where
        F: FnMut() -> Self::Request + Send + 'static,
    {
        self.spawn_with_capacity_periodic(DEFAULT_TASK_CAPACITY, every, make_request)
    }
}
