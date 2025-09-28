mod coordinator;
pub mod error;
pub mod types;
pub use coordinator::{ProofCoordinator, spawn_forwarder};

pub mod prelude {
    pub use crate::{
        error::{Error, Result},
        types::*,
    };
}
