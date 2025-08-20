mod rpc;
mod cli;
mod client;
mod message;
mod server;
mod types;

pub use cli::*;
pub use client::{ProverClient, ProverClientError};
pub use server::ProverServer;

// TODOs: rm message, client 
//        change server to user mpsc?