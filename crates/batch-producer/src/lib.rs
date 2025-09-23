mod batch_accumulator;
mod batch_producer;
mod error;
pub mod types;
mod utils;

pub use batch_producer::BatchProducer;

pub mod prelude {
    pub use crate::error::{Error, Result};
}
