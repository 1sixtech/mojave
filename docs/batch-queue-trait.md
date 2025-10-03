# BatchQueue Trait

The `BatchQueue` trait provides an interface for sending batches to a queue service (Q).

## Usage

### Implementing the Trait

To create a custom queue implementation, implement the `BatchQueue` trait:

```rust
use mojave_batch_submitter::{types::BatchQueue, error::Result};
use ethrex_common::types::batch::Batch;

pub struct MyQueueService {
    // Your queue service fields
}

impl BatchQueue for MyQueueService {
    fn send_batch(&self, batch: &Batch) -> Result<()> {
        // Your implementation to send batch to queue
        // For example: send to RabbitMQ, Kafka, AWS SQS, etc.
        Ok(())
    }
}
```

### Using with BatchProducer

The `BatchProducer` accepts any implementation of the `BatchQueue` trait:

```rust
use std::sync::Arc;
use mojave_batch_producer::BatchProducer;
use mojave_batch_submitter::queue::NoOpBatchQueue;

// Using the NoOp implementation (default)
let batch_queue = Arc::new(NoOpBatchQueue::new());
let batch_producer = BatchProducer::new(node, 0, batch_queue);

// Or using your custom implementation
let batch_queue = Arc::new(MyQueueService::new());
let batch_producer = BatchProducer::new(node, 0, batch_queue);
```

## Default Implementation

The `NoOpBatchQueue` is provided as a default implementation that logs batch information but doesn't actually send batches anywhere. This is useful for:

- Testing
- Development
- Deployments where queue integration is not needed

## Thread Safety

The `BatchQueue` trait requires implementations to be `Send + Sync`, making them safe to use across async tasks and threads.
