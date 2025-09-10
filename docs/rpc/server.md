 Mojave RPC Server
 ==================

 Transport and routing glue for JSON-RPC 2.0 over HTTP using Axum.

 What it provides
 - A dynamic method registry (`RpcRegistry<C>`) that maps JSON-RPC method
   names (e.g. `"eth_chainId"`, `"moj_getJobId"`) to async handlers.
 - Optional per‑namespace fallbacks (e.g. forward all `eth_*` calls to an L1
   implementation) via `register_fallback`.
 - A small service wrapper (`RpcService<C>`) that binds a context `C` and a
   registry into an Axum `Router` and HTTP server.
 - Batch request support and JSON-RPC error shaping.

 Quick start
 -----------
 ```rust
 # use mojave_rpc_server::{RpcRegistry, RpcService};
 # use mojave_rpc_core::{RpcErr, RpcRequest, types::Namespace};
 # use serde_json::Value;
 # async fn dummy_fallback(_req: &RpcRequest, _ctx: ()) -> Result<Value, RpcErr> { Ok(serde_json::json!(null)) }
 # async fn my_handler(_req: &RpcRequest, _ctx: ()) -> Result<Value, RpcErr> { Ok(serde_json::json!("ok")) }
 let mut registry: RpcRegistry<()> = RpcRegistry::new();
 registry
     .register_fn("moj_echo", |req, ctx| Box::pin(my_handler(req, ctx)))
     .register_fallback(Namespace::Eth, |req, ctx| Box::pin(dummy_fallback(req, ctx)));
 let service = RpcService::new((), registry);
 let _router = service.router(); // attach layers (CORS, tracing, limits) as needed
 ```

 Error shape
 -----------
 Errors returned by handlers are converted into standard JSON-RPC error
 objects using `ethrex_rpc::utils::RpcErr` → `RpcErrorResponse` mapping.
 Bad request bodies or malformed batches return a `BadParams` error payload.
