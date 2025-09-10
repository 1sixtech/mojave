 Mojave RPC Macros
 ==================

 Attribute macro to register typed JSON-RPC handlers with the Mojave RPC server.

 Quick start
 -----------
 1) Annotate a handler that takes a context and a typed parameter:

 ```rust
 use mojave_rpc_core::RpcErr;
 use serde_json::Value;

 #[derive(Clone)]
 struct Ctx;

 #[mojave_rpc_macros::rpc(namespace = "moj", method = "getJobId")]
 pub async fn get_job_id(
     _ctx: Ctx,
     _params: (),
 ) -> Result<Value, RpcErr> {
     Ok(serde_json::json!(["id-1", "id-2"]))
 }
 ```

 2) Register it when building your server:

 ```rust
 # use mojave_rpc_macros as _macro_dep_only;
 # use mojave_rpc_core::RpcErr;
 # use serde_json::Value;
 # #[derive(Clone)] struct Ctx;
 # #[mojave_rpc_macros::rpc(namespace = "moj", method = "getJobId")]
 # async fn get_job_id(_ctx: Ctx, _params: ()) -> Result<Value, RpcErr> { Ok(serde_json::json!([])) }
 let mut registry: mojave_rpc_server::RpcRegistry<Ctx> =
     mojave_rpc_server::RpcRegistry::new();
 register_moj_getJobId(&mut registry);
 ```

 Parameter extraction rules
 --------------------------
 The macro deserializes parameters into your handler type `P` using the
 following rules applied to `req.params`:
 - `None` or empty array `[]` -> `serde_json::from_value::<P>(Null)`
 - Single element array `[x]` -> `serde_json::from_value::<P>(x)`
 - Multiple elements array `[x, y, ...]` -> `serde_json::from_value::<P>(Array)`

 This enables three common patterns:
 - Zero parameters: use `()`.
 - Single parameter: use the concrete type (e.g., `String`, a DTO, ...).
 - Multiple parameters: use a tuple (e.g., `(A, B)`) or an enum/struct
   designed to capture the array shape.

 Tip: for backwards‑compatible APIs accept both shapes using an
 `#[serde(untagged)]` enum, e.g.:

 ```rust
 #[derive(serde::Serialize, serde::Deserialize)]
 #[serde(deny_unknown_fields)]
 pub struct MyDto { pub a: u64, pub b: String }

 #[derive(serde::Serialize, serde::Deserialize)]
 #[serde(untagged)]
 pub enum MyParam { Object(MyDto), Tuple((u64, String)) }
 ```

 Error handling
 --------------
 - Any deserialization failure returns `RpcErr::BadParams("Invalid params: …")`.
 - Handlers return `Result<Value, RpcErr>`; errors propagate to the JSON-RPC error
   object via the server glue.

 Generated symbols
 -----------------
 - For `#[rpc(namespace = "ns", method = "foo")] fn handler(...)`, the macro
   generates: `fn register_ns_foo(registry: &mut RpcRegistry<C>)`.
 - Call this registrar to add your handler to the dynamic registry.

 Notes on performance
 --------------------
 - The dynamic registry uses a boxed future internally to erase types. This is the
   idiomatic, low‑overhead approach for dynamic routing; the overhead is typically
   negligible compared to JSON parsing and I/O.

 Requirements
 ------------
 - The using crate must depend on `serde_json`, `mojave-rpc-core`, and
   `mojave-rpc-server`.
 - Your parameter types must implement `serde::Deserialize` (and `Serialize` if
   you return them directly as part of the response body).
