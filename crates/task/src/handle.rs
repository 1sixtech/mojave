use crate::{Error, Task};
use tokio::sync::{mpsc, oneshot};

pub struct TaskHandle<T>
where
    T: Task,
{
    request: mpsc::Sender<(T::Request, oneshot::Sender<Result<T::Response, T::Error>>)>,
    shutdown: mpsc::Sender<oneshot::Sender<Result<(), T::Error>>>,
}

impl<T: Task> Clone for TaskHandle<T> {
    fn clone(&self) -> Self {
        Self {
            request: self.request.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

impl<T> TaskHandle<T>
where
    T: Task,
{
    pub(crate) fn new(
        request: mpsc::Sender<(T::Request, oneshot::Sender<Result<T::Response, T::Error>>)>,
        shutdown: mpsc::Sender<oneshot::Sender<Result<(), T::Error>>>,
    ) -> Self {
        Self { request, shutdown }
    }

    pub async fn request(
        &self,
        request: T::Request,
    ) -> Result<Result<T::Response, T::Error>, Error> {
        let (sender, receiver) = oneshot::channel();
        self.request
            .send((request, sender))
            .await
            .map_err(|error| Error::Send(error.to_string()))?;
        let result = receiver.await?;
        Ok(result)
    }

    pub async fn shutdown(&self) -> Result<Result<(), T::Error>, Error> {
        let (sender, receiver) = oneshot::channel();
        self.shutdown
            .send(sender)
            .await
            .map_err(|error| Error::Send(error.to_string()))?;
        let result = receiver.await?;
        Ok(result)
    }
}
