#[cfg(feature = "client")]
mod client;
mod message;
mod request;
mod response;
#[cfg(feature = "server")]
mod server;

#[cfg(feature = "client")]
pub use client::{ProverClient, ProverClientError};
#[cfg(feature = "server")]
pub use server::ProverServer;

pub use request::ProverData;
