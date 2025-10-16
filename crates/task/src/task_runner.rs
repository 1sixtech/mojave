use crate::traits::Task;
use tokio::sync::{mpsc, oneshot};

pub type RequestSignal<T> = (
    <T as Task>::Request,
    oneshot::Sender<Result<<T as Task>::Response, <T as Task>::Error>>,
);
pub type ShutdownSignal<T> = oneshot::Sender<Result<(), <T as Task>::Error>>;

pub struct TaskRunner<T: Task + 'static> {
    request: mpsc::Receiver<RequestSignal<T>>,
    shutdown: mpsc::Receiver<ShutdownSignal<T>>,
    task: T,
}

impl<T: Task + 'static> TaskRunner<T> {
    pub fn new(
        request: mpsc::Receiver<RequestSignal<T>>,
        shutdown: mpsc::Receiver<ShutdownSignal<T>>,
        task: T,
    ) -> Self {
        Self {
            request,
            shutdown,
            task,
        }
    }

    pub async fn listen(&mut self) {
        if let Err(error) = self.task.on_start().await {
            tracing::error!(
                "Error while start task '{}'. Message: {}",
                self.task.name(),
                error
            )
        }
        loop {
            tokio::select! {
                request = self.request.recv() => {
                    if let Some((request, sender)) = request {
                        self.task.on_request_started(&request);
                        let response = self.task.handle_request(request).await;
                        self.task.on_request_finished(&response);
                        let _ = sender.send(response);
                    }
                }
                shutdown = self.shutdown.recv() => {
                    if let Some(sender) = shutdown {
                        let response = self.task.on_shutdown().await;
                        let _ = sender.send(response);
                        return;
                    }
                }
            }
        }
    }
}
