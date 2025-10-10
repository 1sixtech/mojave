Mojave Task
===
The crate helps define and run asynchronous tasks in a pattern similar to that of client-server.

## Quickstart
1. Implement `Task` trait and define associated types.
```rust
pub struct BlockProducer {
    // Block number,
    context: std::sync::atomic::AtomicUsize,
}

impl mojave_task::Task for BlockProducer {
    type Request = Request;
    type Response = Response;
    type Error = Error;
}

pub enum Request {
    BuildBlock,
    BuildBatch,
}

pub enum Response {
    Block(usize)
}

#[thiserror::Error, Debug]
pub enum Error {
    Unsupported,
}
```

2. Define the handler which defines action branches on `Request`.
```rust
impl mojave_task::Task for BlockProducer {
    type Request = Request;
    type Response = Response;
    type Error = Error;

    async fn handle_request(
        &self,
        request: Self::Request,
    ) -> Result<Self::Response, Self::Error> {
        match request {
            // Increase the block number by 1 on `Request::BuildBlock`.
            Request::BuildBlock => Ok(Response::Block(
                self.context
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            )),
            Request::BuildBatch => Err(Error::Unsupported)
        }
    }
}
```

3. (Optional) Override lifecycle hooks.
By default, `on_start` and `on_shutdown` are no-ops that return `Ok(())`. Override them only if you need custom behavior.
```rust
impl mojave_task::Task for BlockProducer {
    async fn handle_request(...
    
    async fn on_shutdown(&self) -> Result<(), Self::Error> {
        println!("Shutting down the block producer..");
        Ok(())
    }
}
```

4. Run the task by calling `spawn()` on the type you implemented `Task` for.
```rust
#[tokio::main]
async fn main() {
    let block_producer = BlockProducer::new();
    // Spawn an async task which serves requests sent by the handle.
    let block_producer_handle = block_producer.spawn();

    // Spawn another task which send `Request::BuildBlock` every 1 second.
    let handle = block_producer_handle.clone();
    tokio::spawn(async move {
        loop {
            let response = handle.request(Request::BuildBlock).await.unwrap();
            if let Response::Block(value) = response {
                println!("Successfully built the block (number {value})");
            }
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }
    });

    // Signal the task to shutdown on `SIGINT`.
    tokio::signal::ctrl_c().await.unwrap();
    handle.shutdown().await.unwrap();
}
```
