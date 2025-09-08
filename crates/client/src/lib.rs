mod client;
pub mod error;
pub mod types;

pub use client::MojaveClient;

pub mod prelude {
    pub use crate::error::{Error, Result};
    pub use crate::types::*;
}
