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
        loop {
            tokio::select! {
                request = self.request.recv() => {
                    if let Some((request, sender)) = request {
                        let response = self.task.handle_request(request).await;
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
