mod constants;
mod error;
mod handle;
mod runner;
mod traits;

pub use constants::*;
pub use error::Error;
pub use handle::TaskHandle;
pub use traits::Task;

#[tokio::test]
async fn works() {
    let handle = BlockProducer::new().spawn();
    for i in 0..5 {
        let response = handle.request(Request::BuildBlock).await.unwrap();
        if let Response::Block(value) = response {
            assert!(value == i);
        }
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
    handle.shutdown().await.unwrap();

    pub struct BlockProducer {
        block_number: std::sync::atomic::AtomicUsize,
    }

    impl Task for BlockProducer {
        type Request = Request;
        type Response = Response;
        type Error = Error;

        async fn on_start(&mut self) -> Result<(), Self::Error> {
            println!("Starting BlockProducer test task");
            Ok(())
        }

        fn on_request_started(&mut self, req: &Self::Request) {
            println!("Request {req:?} started");
        }

        fn on_request_finished(&mut self, res: &Result<Self::Response, Self::Error>) {
            println!("Request finished. result: {res:?}");
        }

        async fn handle_request(
            &mut self,
            request: Self::Request,
        ) -> Result<Self::Response, Self::Error> {
            match request {
                Request::BuildBlock => Ok(Response::Block(
                    self.block_number
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                )),
            }
        }

        async fn on_shutdown(&mut self) -> Result<(), Self::Error> {
            println!("Shutting down the block producer..");
            Ok(())
        }
    }

    impl BlockProducer {
        pub fn new() -> Self {
            Self {
                block_number: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[derive(Debug)]
    pub enum Request {
        BuildBlock,
    }

    #[allow(unused)]
    #[derive(Debug)]
    pub enum Response {
        Block(usize),
        PlaceHolder,
    }

    #[derive(thiserror::Error, Debug)]
    pub enum Error {}
}
