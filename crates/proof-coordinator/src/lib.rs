mod context;
mod coordinator;
pub mod error;
pub mod types;
pub use context::ProofCoordinatorContext;
pub use coordinator::{ProofCoordinator, run};

pub mod prelude {
    pub use crate::{
        error::{Error, Result},
        types::*,
    };
}
