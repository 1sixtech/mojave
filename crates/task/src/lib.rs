mod error;
mod handle;
mod runner;
mod traits;

pub use error::Error;
pub use handle::TaskHandle;
pub use traits::Task;

/// TODO: Remove after documentation.
#[allow(unused)]
#[tokio::test(flavor = "multi_thread")]
async fn works() {
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    pub struct BlockProducer {
        block_number: AtomicUsize,
    }

    impl Task for BlockProducer {
        type Request = Request;
        type Response = Response;
        type Error = Error;

        async fn handle_request(
            &self,
            request: Self::Request,
        ) -> Result<Self::Response, Self::Error> {
            match request {
                Request::BuildBlock => Ok(Response::Block(
                    self.block_number.fetch_add(1, Ordering::SeqCst).to_string(),
                )),
            }
        }

        async fn on_shutdown(&self) -> Result<(), Self::Error> {
            println!("Shutting down the block producer..");
            Ok(())
        }
    }

    impl BlockProducer {
        pub fn new() -> Self {
            Self {
                block_number: AtomicUsize::new(0),
            }
        }
    }

    pub enum Request {
        BuildBlock,
    }

    #[derive(Debug)]
    pub enum Response {
        Block(String),
    }

    #[derive(thiserror::Error, Debug)]
    pub enum Error {}

    // Entry
    let handle = BlockProducer::new().spawn();
    tokio::spawn({
        let handle = handle.clone();
        async move {
            loop {
                let response = handle.request(Request::BuildBlock).await.unwrap().unwrap();
                println!("Built block: {response:?}");
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
        }
    });

    tokio::signal::ctrl_c().await.unwrap();
    handle.shutdown().await.unwrap().unwrap();
}
