#[cfg(feature = "secp256k1")]
pub mod ecdsa;
#[cfg(feature = "ed25519")]
pub mod eddsa;
pub mod error;
pub mod types;

cfg_if::cfg_if! {
    if #[cfg(feature = "secp256k1")] {
      pub use ecdsa::{SigningKey, VerifyingKey};
    } else if #[cfg(feature = "ed25519")] {
      pub use eddsa::{SigningKey, VerifyingKey};
    }
}

pub mod prelude {
    pub use crate::error::{Error, Result};
    pub use crate::types::*;
}
