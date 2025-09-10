#![doc = include_str!("../../../../docs/rpc/server.md")]
use std::{collections::HashMap, future::Future, net::SocketAddr, pin::Pin, sync::Arc};

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use ethrex_rpc::RpcRequestWrapper;
use mojave_rpc_core::{
    RpcErr, RpcRequest,
    types::Namespace,
    utils::{resolve_namespace, rpc_response},
};
use serde_json::Value;
use tower_http::cors::CorsLayer;
use tracing::info;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub type RpcResult = Result<Value, RpcErr>;

type DynHandler<C> =
    Arc<dyn for<'a> Fn(&'a RpcRequest, C) -> BoxFuture<'a, RpcResult> + Send + Sync + 'static>;

#[derive(Clone)]
pub struct RpcRegistry<C> {
    handlers: HashMap<String, DynHandler<C>>,
    fallbacks: HashMap<Namespace, DynHandler<C>>,
}

impl<C> Default for RpcRegistry<C> {
    fn default() -> Self {
        Self {
            handlers: HashMap::new(),
            fallbacks: HashMap::new(),
        }
    }
}

impl<C: Clone + Send + Sync + 'static> RpcRegistry<C> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_fn<F>(&mut self, method: &str, f: F) -> &mut Self
    where
        F: for<'a> Fn(&'a RpcRequest, C) -> BoxFuture<'a, RpcResult> + Send + Sync + 'static,
    {
        let func: DynHandler<C> = Arc::new(move |req, ctx: C| f(req, ctx));
        self.handlers.insert(method.to_string(), func);
        self
    }

    pub fn register_fallback<F>(&mut self, ns: Namespace, f: F) -> &mut Self
    where
        F: for<'a> Fn(&'a RpcRequest, C) -> BoxFuture<'a, RpcResult> + Send + Sync + 'static,
    {
        let func: DynHandler<C> = Arc::new(move |req, ctx: C| f(req, ctx));
        self.fallbacks.insert(ns, func);
        self
    }

    pub fn with_handler<F>(mut self, method: &str, f: F) -> Self
    where
        F: for<'a> Fn(&'a RpcRequest, C) -> BoxFuture<'a, RpcResult> + Send + Sync + 'static,
    {
        self.register_fn(method, f);
        self
    }

    pub fn with_fallback<F>(mut self, ns: Namespace, f: F) -> Self
    where
        F: for<'a> Fn(&'a RpcRequest, C) -> BoxFuture<'a, RpcResult> + Send + Sync + 'static,
    {
        self.register_fallback(ns, f);
        self
    }

    async fn dispatch(&self, req: &RpcRequest, ctx: C) -> RpcResult {
        tracing::debug!(method = %req.method, id = ?req.id, "Dispatching RPC request");

        let start = std::time::Instant::now();
        let result = if let Some(handler) = self.handlers.get(&req.method) {
            handler(req, ctx).await
        } else {
            match resolve_namespace(req) {
                Ok(ns) => {
                    if let Some(fallback) = self.fallbacks.get(&ns) {
                        fallback(req, ctx).await
                    } else {
                        Err(RpcErr::MethodNotFound(req.method.clone()))
                    }
                }
                Err(err) => Err(err),
            }
        };

        let duration = start.elapsed();
        match &result {
            Ok(_) => {
                tracing::debug!(method = %req.method, duration_ms = duration.as_millis(), "RPC request completed")
            }
            Err(e) => {
                tracing::warn!(method = %req.method, error = %e, duration_ms = duration.as_millis(), "RPC request failed")
            }
        }

        result
    }
}

/// Service that binds a context and registry into an Axum router.
///
/// The router exposes a single POST `/` endpoint that accepts JSON-RPC 2.0
/// single or batch requests. Attach your own layers (CORS, limits, tracing)
/// on the returned `Router`.
#[derive(Clone)]
pub struct RpcService<C> {
    context: C,
    registry: RpcRegistry<C>,
    router: Router,
}

impl<C: Clone + Send + Sync + 'static> RpcService<C> {
    pub fn new(context: C, registry: RpcRegistry<C>) -> Self {
        let this = Self {
            context,
            registry,
            router: Router::new(),
        };

        let router = Router::new()
            .route("/", post(handle::<C>))
            .with_state(this.clone());

        Self { router, ..this }
    }

    /// Build an Axum router mounted at `/` with JSON-RPC 2.0 handler.
    #[inline]
    pub fn router(self) -> Router {
        self.router
    }

    #[inline]
    pub fn with_cors(mut self, cors: CorsLayer) -> Self {
        self.router = self.router.layer(cors);
        self
    }

    #[inline]
    pub fn with_permissive_cors(self) -> Self {
        self.with_cors(CorsLayer::permissive())
    }

    pub async fn serve(self, addr: SocketAddr) -> Result<(), RpcErr> {
        let router = self.router();
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| RpcErr::Internal(e.to_string()))?;
        let server = axum::serve(listener, router)
            .with_graceful_shutdown(ethrex_rpc::shutdown_signal())
            .into_future();
        info!("Starting HTTP server at {addr}");
        server.await.map_err(|e| RpcErr::Internal(e.to_string()))
    }
}

async fn handle<C: Clone + Send + Sync + 'static>(
    State(service): State<RpcService<C>>,
    body: String,
) -> core::result::Result<Json<Value>, (StatusCode, Json<Value>)> {
    let wrapper = match serde_json::from_str::<RpcRequestWrapper>(&body) {
        Ok(wrapper) => wrapper,
        Err(_) => {
            let error_response = rpc_response(
                mojave_rpc_core::RpcRequestId::Number(0),
                Err(RpcErr::BadParams("Invalid JSON".to_string())),
            )
            .unwrap_or_else(|_| serde_json::json!({"error": "Parse error"}));
            return Err((StatusCode::BAD_REQUEST, Json(error_response)));
        }
    };

    let res = match wrapper {
        RpcRequestWrapper::Single(request) => {
            let res = service
                .registry
                .dispatch(&request, service.context.clone())
                .await;
            rpc_response(request.id, res)
                .unwrap_or_else(|_| serde_json::json!({"error": "Response serialization failed"}))
        }
        RpcRequestWrapper::Multiple(requests) => {
            let responses: Vec<_> = futures::future::join_all(requests.into_iter().map(|req| {
                let registry = &service.registry;
                let context = service.context.clone();
                async move {
                    let res = registry.dispatch(&req, context).await;
                    rpc_response(req.id, res).unwrap_or_else(
                        |_| serde_json::json!({"error": "Response serialization failed"}),
                    )
                }
            }))
            .await;
            serde_json::to_value(responses)
                .unwrap_or_else(|_| serde_json::json!({"error": "Batch serialization failed"}))
        }
    };

    Ok(Json(res))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dispatch_hits_registered_handler() {
        let mut reg: RpcRegistry<()> = RpcRegistry::new();
        reg.register_fn("eth_chainId", |_req, _ctx| {
            Box::pin(async { Ok(serde_json::json!("0x1")) })
        });
        let req: mojave_rpc_core::RpcRequest =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}"#)
                .unwrap();
        let out = reg.dispatch(&req, ()).await.unwrap();
        assert_eq!(out, serde_json::json!("0x1"));
    }

    #[tokio::test]
    async fn dispatch_uses_fallback() {
        let mut reg: RpcRegistry<()> = RpcRegistry::new();
        reg.register_fallback(Namespace::Eth, |_req, _ctx| {
            Box::pin(async { Ok(serde_json::json!("ok")) })
        });
        let req: mojave_rpc_core::RpcRequest = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}"#,
        )
        .unwrap();
        let out = reg.dispatch(&req, ()).await.unwrap();
        assert_eq!(out, serde_json::json!("ok"));
    }

    #[tokio::test]
    async fn dispatch_method_not_found_without_fallback() {
        let reg: RpcRegistry<()> = RpcRegistry::new();
        let req: mojave_rpc_core::RpcRequest = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}"#,
        )
        .unwrap();
        let err = reg.dispatch(&req, ()).await.err().unwrap();
        match err {
            mojave_rpc_core::RpcErr::MethodNotFound(m) => assert_eq!(m, "eth_blockNumber"),
            _ => panic!("unexpected error"),
        }
    }

    #[tokio::test]
    async fn handle_batch_requests() {
        let mut reg: RpcRegistry<()> = RpcRegistry::new();
        reg.register_fn("moj_echo", |req, _| {
            Box::pin(async move { Ok(serde_json::to_value(&req.params).unwrap()) })
        });
        let service = RpcService::new((), reg);
        let body = r#"[
            {"jsonrpc":"2.0","id":1,"method":"moj_echo","params":["a"]},
            {"jsonrpc":"2.0","id":2,"method":"moj_echo","params":["b"]}
            ]"#;
        let Json(val) = super::handle::<_>(axum::extract::State(service), body.into())
            .await
            .unwrap();
        assert!(val.is_array());
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }
}
