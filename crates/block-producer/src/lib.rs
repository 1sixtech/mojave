mod block_producer;
mod error;
mod service;

pub mod types;

pub use block_producer::BlockProducer;
pub use service::run;

pub mod prelude {
    pub use crate::{
        error::{Error, Result},
        types::*,
    };
}
