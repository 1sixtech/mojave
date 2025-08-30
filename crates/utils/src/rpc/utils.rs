use crate::rpc::{
    error::{Error, Result},
    types::{Namespace, RpcErrorResponse, RpcRequest, RpcRequestId, RpcSuccessResponse},
};
use serde_json::Value;

pub fn rpc_response(id: RpcRequestId, res: Result<Value>) -> Result<Value> {
    Ok(match res {
        Ok(result) => serde_json::to_value(RpcSuccessResponse {
            id,
            jsonrpc: "2.0".to_string(),
            result,
        }),
        Err(error) => serde_json::to_value(RpcErrorResponse {
            id,
            jsonrpc: "2.0".to_string(),
            error: error.into(),
        }),
    }?)
}

pub fn resolve_namespace(req: &RpcRequest) -> Result<Namespace> {
    let req_method = req.method.replace('\"', "");
    let mut parts = req_method.split('_');
    let Some(namespace) = parts.next() else {
        return Err(Error::MethodNotFound(req.method.clone()));
    };
    match namespace {
        "debug" => Ok(Namespace::Debug),
        "eth" => Ok(Namespace::Eth),
        "moj" => Ok(Namespace::Mojave),
        "net" => Ok(Namespace::Net),
        "txpool" => Ok(Namespace::TxPool),
        "web3" => Ok(Namespace::Web3),
        _others => Err(Error::MethodNotFound(req.method.clone())),
    }
}
