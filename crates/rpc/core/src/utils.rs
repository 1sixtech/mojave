use crate::{
    RpcErr, RpcRequest, RpcRequestId, RpcSuccessResponse,
    types::{Namespace, RpcErrorResponse},
};
use serde_json::Value;

pub fn rpc_response(id: RpcRequestId, result: Result<Value, RpcErr>) -> Result<Value, RpcErr> {
    match result {
        Ok(value) => rpc_response_success(id, value),
        Err(e) => rpc_response_error(Some(id), e),
    }
}

pub fn rpc_response_success(id: RpcRequestId, result: Value) -> Result<Value, RpcErr> {
    Ok(serde_json::to_value(RpcSuccessResponse {
        id,
        jsonrpc: "2.0".to_string(),
        result,
    })?)
}

pub fn rpc_response_error(id: Option<RpcRequestId>, error: RpcErr) -> Result<Value, RpcErr> {
    Ok(serde_json::to_value(RpcErrorResponse {
        jsonrpc: "2.0".to_string(),
        id,
        error: error.into(),
    })?)
}

pub fn resolve_namespace(req: &RpcRequest) -> Result<Namespace, RpcErr> {
    let req_method = req.method.replace('\"', "");
    let mut parts = req_method.split('_');
    let Some(namespace) = parts.next() else {
        return Err(RpcErr::MethodNotFound(req.method.clone()));
    };
    match namespace {
        "debug" => Ok(Namespace::Debug),
        "eth" => Ok(Namespace::Eth),
        "moj" => Ok(Namespace::Mojave),
        "net" => Ok(Namespace::Net),
        "txpool" => Ok(Namespace::TxPool),
        "web3" => Ok(Namespace::Web3),
        _others => Err(RpcErr::MethodNotFound(req.method.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn success_requires_id_and_echoes_it() {
        let out = rpc_response_success(RpcRequestId::Number(42), json!("ok")).unwrap();
        assert!(out.is_object());
        let obj = out.as_object().unwrap();
        assert_eq!(obj.get("jsonrpc").and_then(|v| v.as_str()), Some("2.0"));
        assert_eq!(obj.get("id").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(obj.get("result"), Some(&json!("ok")));
    }

    #[test]
    fn error_with_id_keeps_id() {
        let out = rpc_response_error(Some(RpcRequestId::Number(7)), RpcErr::BadParams("x".into()))
            .unwrap();
        let obj = out.as_object().expect("object response");
        assert_eq!(obj.get("jsonrpc").and_then(|v| v.as_str()), Some("2.0"));
        assert_eq!(obj.get("id").and_then(|v| v.as_i64()), Some(7));
        assert!(obj.get("error").is_some());
        assert!(obj.get("result").is_none());
    }

    #[test]
    fn error_without_id_sets_null_id() {
        let out = rpc_response_error(None, RpcErr::BadParams("y".into())).unwrap();
        let obj = out.as_object().expect("object response");
        assert_eq!(obj.get("jsonrpc").and_then(|v| v.as_str()), Some("2.0"));
        assert!(obj.get("id").unwrap().is_null());
        assert!(obj.get("error").is_some());
        assert!(obj.get("result").is_none());
    }
}
