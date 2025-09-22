use crate::{
    error::Error,
    runner::{RequestSignal, ShutdownSignal},
    traits::Task,
};
use tokio::sync::{mpsc, oneshot};

pub struct TaskHandle<T>
where
    T: Task,
{
    request: mpsc::Sender<RequestSignal<T>>,
    shutdown: mpsc::Sender<ShutdownSignal<T>>,
}

impl<T: Task> Clone for TaskHandle<T> {
    fn clone(&self) -> Self {
        Self {
            request: self.request.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

impl<T: Task> Drop for TaskHandle<T> {
    fn drop(&mut self) {
        let (sender, receiver) = oneshot::channel();
        let shutdown = self.shutdown.clone();
        tokio::spawn(async move {
            // If the receiver is closed, self.shutdown() has already taken place.
            // Therefore we only deal with successful send.
            if let Ok(()) = shutdown.send(sender).await
                && let Err(error) = receiver.await.unwrap()
            {
                tracing::error!("{error}");
            }
        });
    }
}

impl<T> TaskHandle<T>
where
    T: Task,
{
    pub(crate) fn new(
        request: mpsc::Sender<RequestSignal<T>>,
        shutdown: mpsc::Sender<ShutdownSignal<T>>,
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
