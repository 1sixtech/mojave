pub mod types;
pub mod utils;

pub use ethrex_rpc::utils::{
    RpcErr, RpcErrorResponse, RpcRequest, RpcRequestId, RpcSuccessResponse,
};

pub mod prelude {
    pub use crate::{types::*, utils::*};
    pub use ethrex_rpc::utils::{RpcErr, RpcRequest, RpcRequestId};
}
