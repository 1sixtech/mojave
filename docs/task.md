Mojave Task
===
The crate helps define and run asynchronous tasks in a pattern similar to that of client-server with comprehensive lifecycle management.

## Quickstart
1. Implement `Task` trait and define associated types.
```rust
use std::sync::atomic::AtomicUsize;

pub struct BlockProducer {
    block_number: AtomicUsize,
}

impl mojave_task::Task for BlockProducer {
    type Request = Request;
    type Response = Response;
    type Error = Error;
}

#[derive(Debug)]
pub enum Request {
    BuildBlock,
    BuildBatch,
}

#[derive(Debug)]
pub enum Response {
    Block(usize),
    PlaceHolder,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Unsupported operation")]
    Unsupported,
    #[error("Internal error: {0}")]
    Internal(String),
}
```

2. (Optional) Define the complete task lifecycle including startup, request handling, and shutdown.
```rust
impl mojave_task::Task for BlockProducer {
    type Request = Request;
    type Response = Response;
    type Error = Error;

    // Called once when the task starts
    async fn on_start(&mut self) -> Result<(), Self::Error> {
        println!("Starting BlockProducer task");
        Ok(())
    }

    // Called before each request is processed
    fn on_request_started(&mut self, req: &Self::Request) {
        println!("Processing request: {req:?}");
    }

    // Called after each request is processed
    fn on_request_finished(&mut self, res: &Result<Self::Response, Self::Error>) {
        match res {
            Ok(response) => println!("Request completed successfully: {response:?}"),
            Err(error) => println!("Request failed: {error}"),
        }
    }

    // Main request handler - defines action branches on `Request`
    async fn handle_request(
        &mut self,
        request: Self::Request,
    ) -> Result<Self::Response, Self::Error> {
        match request {
            Request::BuildBlock => {
                let block_num = self.block_number.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(Response::Block(block_num))
            }
            Request::BuildBatch => Err(Error::Unsupported),
        }
    }

    // Called when the task is shutting down
    async fn on_shutdown(&mut self) -> Result<(), Self::Error> {
        println!("Shutting down the block producer..");
        Ok(())
    }
}
```

3. Implement associated functions for the task struct.
```rust
impl BlockProducer {
    pub fn new() -> Self {
        Self {
            block_number: AtomicUsize::new(0),
        }
    }
}
```

4. Run the task by calling `spawn()` on the type you implemented `Task` for.
```rust
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let block_producer = BlockProducer::new();

    // Spawn an async task which serves requests sent by the handle
    let block_producer_handle = block_producer.spawn();

    // Spawn another task which sends `Request::BuildBlock` every 1 second
    let handle = block_producer_handle.clone();
    tokio::spawn(async move {
        loop {
            match handle.request(Request::BuildBlock).await {
                Ok(Response::Block(value)) => {
                    println!("Successfully built block (number {value})");
                }
                Ok(Response::PlaceHolder) => {
                    println!("Received placeholder response");
                }
                Err(error) => {
                    eprintln!("Error building block: {error}");
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });

    // Wait for shutdown signal and then shutdown the task
    tokio::signal::ctrl_c().await?;
    block_producer_handle.shutdown().await?;

    Ok(())
}
```

## Advanced Usage

### Custom Task Capacity
By default, the task uses a channel capacity of 64 for both requests and shutdown signals. You can customize this:

```rust
// Use custom capacity for high-throughput scenarios
let handle = block_producer.spawn_with_capacity(1024);
```

### Periodic Tasks
You can spawn a task that automatically sends requests at regular intervals:

```rust
use std::time::Duration;

// Spawn a task that automatically builds blocks every 500ms
let handle = block_producer.spawn_periodic(
    Duration::from_millis(500),
    || Request::BuildBlock,  // Closure that generates the request
);
```

### Task Lifecycle Callbacks
The `Task` trait provides several lifecycle callbacks:

- `on_start()`: Called once when the task starts up
- `on_request_started()`: Called before processing each request
- `on_request_finished()`: Called after processing each request (with the result)
- `on_shutdown()`: Called when the task is shutting down

### Error Handling
The crate provides comprehensive error handling:

```rust
use mojave_task::Error;

match handle.request(Request::BuildBlock).await {
    Ok(response) => println!("Success: {response:?}"),
    Err(Error::Send(msg)) => eprintln!("Failed to send request: {msg}"),
    Err(Error::Receive(_)) => eprintln!("Failed to receive response"),
    Err(Error::Task(task_error)) => eprintln!("Task error: {task_error}"),
}
```

### Task Naming
Each task has a default name based on its type, accessible via the `name()` method:

```rust
let task_name = block_producer.name();  // Returns the type name as &'static str
```
