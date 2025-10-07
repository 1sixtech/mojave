use mojave_utils::signal::wait_for_shutdown_signal;
use tokio_util::sync::CancellationToken;

/// A service that can be run and gracefully shutdown.
#[trait_variant::make(Send)]
pub trait Service {
    type Error: std::error::Error + Send + 'static;

    /// Run the service. Called repeatedly until shutdown or error.
    async fn run(&mut self) -> Result<(), Self::Error>;

    /// Gracefully shutdown the service.
    async fn shutdown(&self) -> Result<(), Self::Error>;
}

/// A runner that manages a service with graceful shutdown support.
///
/// Continuously runs the service until shutdown is triggered by:
/// - System shutdown signal (SIGTERM/SIGINT)
/// - Cancellation token
/// - Service error
pub struct Runner<T: Service> {
    service: T,
    cancel_token: CancellationToken,
}

impl<T: Service> Runner<T> {
    /// Create a new runner with the given service and cancellation token.
    pub fn new(service: T, cancel_token: CancellationToken) -> Self {
        Self {
            service,
            cancel_token,
        }
    }

    /// Spawn the runner in a background task using tokio.
    pub fn spawn(mut self) -> tokio::task::JoinHandle<Result<(), T::Error>>
    where
        T: 'static,
    {
        tokio::spawn(async move { self.run().await })
    }

    /// Run the service until an error, shutdown, or cancellation is triggered.
    pub async fn run(&mut self) -> Result<(), T::Error> {
        loop {
            tokio::select! {
                result = self.service.run() => {
                    result?;
                }
                _ = wait_for_shutdown_signal() => {
                    self.cancel_token.cancel();
                    self.service.shutdown().await?;
                    return Ok(());
                }
                _ = self.cancel_token.cancelled() => {
                    self.service.shutdown().await?;
                    return Ok(());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[derive(Debug)]
    struct TestError;

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Test error")
        }
    }

    impl std::error::Error for TestError {}

    struct MockService {
        should_error: bool,
        run_forever: bool,
        run_called: Arc<AtomicBool>,
        shutdown_called: Arc<AtomicBool>,
    }

    impl MockService {
        fn new() -> Self {
            Self {
                should_error: false,
                run_forever: true,
                run_called: Arc::new(AtomicBool::new(false)),
                shutdown_called: Arc::new(AtomicBool::new(false)),
            }
        }

        fn with_error() -> Self {
            Self {
                should_error: true,
                run_forever: false,
                run_called: Arc::new(AtomicBool::new(false)),
                shutdown_called: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    impl Service for MockService {
        type Error = TestError;
        async fn run(&mut self) -> Result<(), Self::Error> {
            self.run_called.store(true, Ordering::SeqCst);

            if self.should_error {
                return Err(TestError);
            }

            if self.run_forever {
                std::future::pending().await
            } else {
                Ok(())
            }
        }

        async fn shutdown(&self) -> Result<(), Self::Error> {
            self.shutdown_called.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_runner_shutdown_via_cancel_token() {
        let service = MockService::new();
        let run_called = service.run_called.clone();
        let shutdown_called = service.shutdown_called.clone();
        let cancel_token = CancellationToken::new();
        let runner = Runner::new(service, cancel_token.clone());

        let runner_task = runner.spawn();

        // Give it a moment to start running
        tokio::task::yield_now().await;

        // Verify the service started running
        assert!(run_called.load(Ordering::SeqCst));

        // Now cancel it
        cancel_token.cancel();
        let result = runner_task.await.unwrap();

        assert!(result.is_ok());
        assert!(shutdown_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_runner_service_error() {
        let service = MockService::with_error();
        let run_called = service.run_called.clone();
        let shutdown_called = service.shutdown_called.clone();
        let cancel_token = CancellationToken::new();
        let mut runner = Runner::new(service, cancel_token);

        let result = runner.run().await;

        assert!(result.is_err());
        assert!(run_called.load(Ordering::SeqCst));
        assert!(!shutdown_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_runner_long_running_service_cancelled() {
        let service = MockService::new();
        let run_called = service.run_called.clone();
        let shutdown_called = service.shutdown_called.clone();
        let cancel_token = CancellationToken::new();
        let runner = Runner::new(service, cancel_token.clone());

        let runner_task = runner.spawn();

        // Give it a moment to start running
        tokio::task::yield_now().await;

        // Verify the service started running
        assert!(run_called.load(Ordering::SeqCst));

        // Now cancel it
        cancel_token.cancel();

        let result = runner_task.await.unwrap();

        assert!(result.is_ok());
        assert!(run_called.load(Ordering::SeqCst));
        assert!(shutdown_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_runner_spawn() {
        let service = MockService::new();
        let run_called = service.run_called.clone();
        let shutdown_called = service.shutdown_called.clone();
        let cancel_token = CancellationToken::new();
        let runner = Runner::new(service, cancel_token.clone());

        let handle = runner.spawn();

        // Give it a moment to start running
        tokio::task::yield_now().await;

        // Verify the service started running
        assert!(run_called.load(Ordering::SeqCst));

        // Cancel and wait for completion
        cancel_token.cancel();
        let result = handle.await.unwrap();

        assert!(result.is_ok());
        assert!(shutdown_called.load(Ordering::SeqCst));
    }
}
