mod block_producer;
mod context;
mod error;

pub mod rpc;
pub mod services;
pub mod types;

pub use block_producer::{BlockProducer, run};
pub use context::BlockProducerContext;

pub mod prelude {
    pub use crate::{
        error::{Error, Result},
        types::*,
    };
}
