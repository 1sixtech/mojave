mod coordinator;
pub mod error;
pub mod types;
pub use coordinator::{ProofCoordinator, run};

pub mod prelude {
    pub use crate::{
        error::{Error, Result},
        types::*,
    };
}
