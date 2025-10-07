use std::sync::Arc;

use crate::{
    error::Error,
    task_runner::{RequestSignal, ShutdownSignal},
    traits::Task,
};
use tokio::sync::{mpsc, oneshot};

pub struct TaskHandle<T: Task> {
    inner: Arc<TaskHandleInner<T>>,
}

struct TaskHandleInner<T: Task> {
    request: mpsc::Sender<RequestSignal<T>>,
    shutdown: mpsc::Sender<ShutdownSignal<T>>,
}

impl<T: Task> Drop for TaskHandleInner<T> {
    fn drop(&mut self) {
        let (sender, receiver) = oneshot::channel();
        let shutdown = self.shutdown.clone();
        tokio::spawn(async move {
            // If the receiver is closed, the task is already down. Therefore we only deal with successful send.
            if let Ok(()) = shutdown.send(sender).await
                && let Err(error) = receiver.await.unwrap()
            {
                tracing::error!("{error}");
            }
        });
    }
}

impl<T: Task> Clone for TaskHandle<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
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
        Self {
            inner: Arc::new(TaskHandleInner { request, shutdown }),
        }
    }

    pub async fn request(&self, request: T::Request) -> Result<T::Response, Error> {
        let (sender, receiver) = oneshot::channel();
        self.inner
            .request
            .send((request, sender))
            .await
            .map_err(|error| Error::Send(error.to_string()))?;
        receiver.await?.map_err(|error| Error::Task(error.into()))
    }

    pub async fn shutdown(&self) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.inner
            .shutdown
            .send(sender)
            .await
            .map_err(|error| Error::Send(error.to_string()))?;
        receiver.await?.map_err(|error| Error::Task(error.into()))
    }
}
