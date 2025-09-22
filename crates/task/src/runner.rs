use crate::{error::Error, traits::Task};
use tokio::sync::{mpsc, oneshot};

pub struct TaskRunner<T: Task + 'static> {
    request: mpsc::Receiver<(T::Request, oneshot::Sender<Result<T::Response, T::Error>>)>,
    shutdown: mpsc::Receiver<oneshot::Sender<Result<(), T::Error>>>,
    task: T,
}

impl<T: Task + 'static> TaskRunner<T> {
    pub fn new(
        request: mpsc::Receiver<(T::Request, oneshot::Sender<Result<T::Response, T::Error>>)>,
        shutdown: mpsc::Receiver<oneshot::Sender<Result<(), T::Error>>>,
        task: T,
    ) -> Self {
        Self {
            request,
            shutdown,
            task,
        }
    }

    pub async fn listen(&mut self) -> Result<(), Error> {
        loop {
            tokio::select! {
                request = self.request.recv() => {
                    if let Some((request, sender)) = request {
                        let response = self.task.handle_request(request).await;
                        let _ = sender.send(response);
                    } else {
                        return Err(Error::TaskHandleDropped(std::any::type_name::<T>()));
                    }
                }
                shutdown = self.shutdown.recv() => {
                    if let Some(sender) = shutdown {
                        let response = self.task.on_shutdown().await;
                        let _ = sender.send(response);
                        return Ok(());
                    } else {
                        return Err(Error::TaskHandleDropped(std::any::type_name::<T>()));
                    }
                }
            }
        }
    }
}
