#[cfg(feature = "server")]
mod cli;
mod message;
#[cfg(feature = "server")]
mod server;
mod types;

#[cfg(feature = "server")]
pub use cli::*;
#[cfg(feature = "client")]
pub use message::MessageError;
#[cfg(feature = "server")]
pub use server::ProverServer;
pub use types::*;
