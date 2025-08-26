pub type Result<T> = std::result::Result<T, Error>;
pub use ethrex_rpc::utils::RpcErr as Error;
