#[cfg(feature = "server")]
mod cli;
#[cfg(feature = "client")]
mod client;
mod message;
#[cfg(feature = "server")]
mod server;
mod types;

#[cfg(feature = "server")]
pub use cli::*;
#[cfg(feature = "client")]
pub use client::{ProverClient, ProverClientError};
#[cfg(feature = "server")]
pub use server::ProverServer;
pub use types::*;
