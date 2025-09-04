pub use mojave_rpc_core::RpcErr as Error;

pub type Result<T> = core::result::Result<T, Error>;
