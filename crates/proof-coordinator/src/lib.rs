mod context;
mod coordinator;
mod error;

pub use context::ProofCoordinatorContext;
pub use coordinator::ProofCoordinator;

pub mod prelude {
    pub use crate::error::{Error, Result};
}
