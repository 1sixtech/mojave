//! Mojave RPC Macros
//! ==================
//!
//! Attribute macro to register typed JSON‑RPC handlers with the Mojave RPC server.
//!
//! Quick start
//! -----------
//! 1) Annotate a handler that takes a context and a typed parameter:
//!
//! ```rust
//! use mojave_rpc_core::RpcErr;
//! use serde_json::Value;
//!
//! #[derive(Clone)]
//! struct Ctx;
//!
//! #[mojave_rpc_macros::rpc(namespace = "moj", method = "getJobId")]
//! pub async fn get_job_id(
//!     _ctx: Ctx,
//!     _params: (),
//! ) -> Result<Value, RpcErr> {
//!     Ok(serde_json::json!(["id-1", "id-2"]))
//! }
//! ```
//!
//! 2) Register it when building your server:
//!
//! ```rust
//! # use mojave_rpc_macros as _macro_dep_only;
//! # use mojave_rpc_core::RpcErr;
//! # use serde_json::Value;
//! # #[derive(Clone)] struct Ctx;
//! # #[mojave_rpc_macros::rpc(namespace = "moj", method = "getJobId")]
//! # async fn get_job_id(_ctx: Ctx, _params: ()) -> Result<Value, RpcErr> { Ok(serde_json::json!([])) }
//! let mut registry: mojave_rpc_server::RpcRegistry<Ctx> =
//!     mojave_rpc_server::RpcRegistry::new();
//! register_moj_getJobId(&mut registry);
//! ```
//!
//! Parameter extraction rules
//! --------------------------
//! The macro deserializes parameters into your handler type `P` using the
//! following rules applied to `req.params`:
//! - `None` or empty array `[]` → `serde_json::from_value::<P>(Null)`
//! - Single element array `[x]` → `serde_json::from_value::<P>(x)`
//! - Multiple elements array `[x, y, ...]` → `serde_json::from_value::<P>(Array)`
//!
//! This enables three common patterns:
//! - Zero parameters: use `()`.
//! - Single parameter: use the concrete type (e.g., `String`, a DTO, ...).
//! - Multiple parameters: use a tuple (e.g., `(A, B)`) or an enum/struct
//!   designed to capture the array shape.
//!
//! Tip: for backwards‑compatible APIs accept both shapes using an
//! `#[serde(untagged)]` enum, e.g.:
//!
//! ```rust
//! #[derive(serde::Serialize, serde::Deserialize)]
//! #[serde(deny_unknown_fields)]
//! pub struct MyDto { pub a: u64, pub b: String }
//!
//! #[derive(serde::Serialize, serde::Deserialize)]
//! #[serde(untagged)]
//! pub enum MyParam { Object(MyDto), Tuple((u64, String)) }
//! ```
//!
//! Error handling
//! --------------
//! - Any deserialization failure returns `RpcErr::BadParams("Invalid params: …")`.
//! - Handlers return `Result<Value, RpcErr>`; errors propagate to the JSON‑RPC error
//!   object via the server glue.
//!
//! Generated symbols
//! -----------------
//! - For `#[rpc(namespace = "ns", method = "foo")] fn handler(...)`, the macro
//!   generates: `fn register_ns_foo(registry: &mut RpcRegistry<C>)`.
//! - Call this registrar to add your handler to the dynamic registry.
//!
//! Notes on performance
//! --------------------
//! - The dynamic registry uses a boxed future internally to erase types. This is the
//!   idiomatic, low‑overhead approach for dynamic routing; the overhead is typically
//!   negligible compared to JSON parsing and I/O.
//!
//! Requirements
//! ------------
//! - The using crate must depend on `serde_json`, `mojave-rpc-core`, and
//!   `mojave-rpc-server`.
//! - Your parameter types must implement `serde::Deserialize` (and `Serialize` if
//!   you return them directly as part of the response body).
//!

use proc_macro::TokenStream;
use proc_macro2::{TokenStream as TokenStream2, TokenTree};
use quote::{ToTokens, format_ident, quote};
use syn::{FnArg, ItemFn, Lit, PatType, Type, parse_macro_input, parse_str};

#[derive(Debug)]
enum ParseError {
    MissingNamespace,
    MissingMethod,
    InvalidFormat(String),
}

fn parse_attr_tokens_panic(ts: TokenStream2) -> (String, String) {
    match parse_attr_tokens(ts) {
        Ok(result) => result,
        Err(ParseError::MissingNamespace) => {
            panic!("#[rpc] requires namespace = \"..\"")
        }
        Err(ParseError::MissingMethod) => {
            panic!("#[rpc] requires method = \"..\"")
        }
        Err(ParseError::InvalidFormat(msg)) => {
            panic!("#[rpc] attribute format error: {msg}")
        }
    }
}

fn parse_attr_tokens(ts: TokenStream2) -> Result<(String, String), ParseError> {
    let mut it = ts.into_iter().peekable();
    let mut namespace = None::<String>;
    let mut method = None::<String>;

    while let Some(tt) = it.next() {
        if let TokenTree::Ident(ident) = tt {
            let key = ident.to_string();

            skip_until_equal(&mut it)?;

            let value = parse_string_literal(&mut it).ok_or_else(|| {
                ParseError::InvalidFormat(format!("Expected string literal after '{key}='"))
            })?;

            match key.as_str() {
                "namespace" => namespace = Some(value),
                "method" => method = Some(value),
                _ => {}
            }
        }
    }

    let ns = namespace.ok_or(ParseError::MissingNamespace)?;
    let m = method.ok_or(ParseError::MissingMethod)?;

    Ok((ns, m))
}

fn parse_string_literal<I>(it: &mut std::iter::Peekable<I>) -> Option<String>
where
    I: Iterator<Item = TokenTree>,
{
    let lit = match it.next()? {
        TokenTree::Literal(l) => l,
        _ => return None,
    };

    match parse_str::<Lit>(&lit.to_string()).ok()? {
        Lit::Str(s) => Some(s.value()),
        other => Some(other.to_token_stream().to_string()),
    }
}

fn skip_until_equal<I>(it: &mut std::iter::Peekable<I>) -> Result<(), ParseError>
where
    I: Iterator<Item = TokenTree>,
{
    const EQUAL: char = '=';

    while let Some(tt) = it.peek() {
        if let TokenTree::Punct(p) = tt
            && p.as_char() == EQUAL
        {
            it.next();
            return Ok(());
        }
        it.next();
    }

    Err(ParseError::InvalidFormat(
        "Expected '=' after attrbute key".to_string(),
    ))
}

fn extract_args_types(input: &ItemFn) -> (Type, Type) {
    let mut it = input.sig.inputs.iter();

    let ctx_arg = it
        .next()
        .expect("RPC handler must take 2 arguments: (ctx, params)");
    let params_arg = it
        .next()
        .expect("RPC handler must take 2 arguments: (ctx, params)");

    if it.next().is_some() {
        panic!("RPC handler must take exactly two arguments: (ctx, params)");
    }

    let extract_type = |arg: &FnArg| -> Type {
        match arg {
            FnArg::Typed(PatType { ty, .. }) => (**ty).clone(),
            _ => panic!("unsupported argument type"),
        }
    };

    let ctx_ty = extract_type(ctx_arg);
    let params_ty = extract_type(params_arg);

    (ctx_ty, params_ty)
}

fn generate_params_parsing(params_type: &Type) -> proc_macro2::TokenStream {
    quote! {
        let params: #params_type = {
            match &req.params {
                None => serde_json::from_value(serde_json::Value::Null)
                    .map_err(|e| mojave_rpc_core::RpcErr::BadParams(format!("Invalid params: {}", e)))?,
                Some(vec) => {
                    match vec.len() {
                        0 => serde_json::from_value(serde_json::Value::Null)
                            .map_err(|e| mojave_rpc_core::RpcErr::BadParams(format!("Invalid params: {}", e)))?,
                        1 => serde_json::from_value::<#params_type>(vec[0].clone())
                            .map_err(|e| mojave_rpc_core::RpcErr::BadParams(format!("Invalid params: {}", e)))?,
                        _ => {
                            let val = serde_json::Value::Array(vec.clone());
                            serde_json::from_value::<#params_type>(val)
                                .map_err(|e| mojave_rpc_core::RpcErr::BadParams(format!("Invalid params: {}", e)))?
                        }
                    }
                }
            }
        };
    }
}

#[proc_macro_attribute]
pub fn rpc(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let (namespace, method) = parse_attr_tokens_panic(attr.into());

    let fn_name = &input.sig.ident;
    let vis = &input.vis;

    let (ctx_type, params_type) = extract_args_types(&input);

    let register_fn = format_ident!("register_{}_{}", namespace, method);
    let full_method = format!("{namespace}_{method}");

    let params_parsing = generate_params_parsing(&params_type);

    let expanded = quote! {
        #input

        #[allow(non_snake_case)]
        #vis fn #register_fn(registry: &mut mojave_rpc_server::RpcRegistry<#ctx_type>) {
            registry.register_fn(#full_method, |req, ctx| {
                Box::pin(async move {
                    #params_parsing
                    #fn_name(ctx, params).await
                })
            });
        }
    };

    TokenStream::from(expanded)
}

#[cfg(test)]
mod tests {
    use crate::{
        ParseError, extract_args_types, generate_params_parsing, parse_attr_tokens,
        parse_attr_tokens_panic,
    };
    use proc_macro2::TokenStream as TokenStream2;
    use quote::{format_ident, quote};
    use syn::{ItemFn, Type, parse_quote};

    #[test]
    fn parse_attr_ok() {
        let ts: TokenStream2 = syn::parse_quote! { namespace = "moj", method = "getJobId" };
        let (ns, m) = parse_attr_tokens_panic(ts);
        assert_eq!(ns, "moj");
        assert_eq!(m, "getJobId");
    }

    #[test]
    fn parse_attr_different_order() {
        let ts: TokenStream2 = syn::parse_quote! { method = "submitJob", namespace = "worker" };
        let (ns, m) = parse_attr_tokens_panic(ts);
        assert_eq!(ns, "worker");
        assert_eq!(m, "submitJob");
    }

    #[test]
    fn parse_attr_with_extra_fields() {
        let ts: TokenStream2 =
            syn::parse_quote! { namespace = "test", method = "call", extra = "ignored" };
        let (ns, m) = parse_attr_tokens_panic(ts);
        assert_eq!(ns, "test");
        assert_eq!(m, "call");
    }

    #[test]
    fn parse_attr_missing_method() {
        let ts: TokenStream2 = syn::parse_quote! { namespace = "test" };
        let result = parse_attr_tokens(ts);
        matches!(result, Err(ParseError::MissingMethod));
    }

    #[test]
    fn parse_attr_empty() {
        let ts: TokenStream2 = syn::parse_quote! {};
        let result = parse_attr_tokens(ts);
        matches!(result, Err(ParseError::MissingNamespace));
    }

    #[test]
    #[should_panic(expected = "#[rpc] requires namespace")]
    fn parse_attr_panic_missing_namespace() {
        let ts: TokenStream2 = syn::parse_quote! { method = "test" };
        parse_attr_tokens_panic(ts);
    }

    #[test]
    #[should_panic(expected = "#[rpc] requires method")]
    fn parse_attr_panic_missing_method() {
        let ts: TokenStream2 = syn::parse_quote! { namespace = "test" };
        parse_attr_tokens_panic(ts);
    }

    #[test]
    fn extract_args_types_basic() {
        let input: ItemFn = parse_quote! {
            async fn test_handler(
                ctx: std::sync::Arc<TestContext>,
                params: String,
            ) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
                Ok(serde_json::Value::Null)
            }
        };

        let (ctx_type, params_type) = extract_args_types(&input);

        let expected_ctx: Type = parse_quote! { std::sync::Arc<TestContext> };
        let expected_params: Type = parse_quote! { String };

        assert_eq!(
            quote!(#ctx_type).to_string(),
            quote!(#expected_ctx).to_string()
        );
        assert_eq!(
            quote!(#params_type).to_string(),
            quote!(#expected_params).to_string()
        );
    }

    #[test]
    fn extract_args_types_unit_params() {
        let input: ItemFn = parse_quote! {
            async fn test_handler(
                ctx: Arc<Context>,
                params: (),
            ) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
                Ok(serde_json::Value::Null)
            }
        };

        let (ctx_type, params_type) = extract_args_types(&input);

        let expected_ctx: Type = parse_quote! { Arc<Context> };
        let expected_params: Type = parse_quote! { () };

        assert_eq!(
            quote!(#ctx_type).to_string(),
            quote!(#expected_ctx).to_string()
        );
        assert_eq!(
            quote!(#params_type).to_string(),
            quote!(#expected_params).to_string()
        );
    }

    #[test]
    fn extract_args_types_tuple_params() {
        let input: ItemFn = parse_quote! {
            async fn test_handler(
                ctx: Context,
                params: (String, u32),
            ) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
                Ok(serde_json::Value::Null)
            }
        };

        let (ctx_type, params_type) = extract_args_types(&input);

        let expected_ctx: Type = parse_quote! { Context };
        let expected_params: Type = parse_quote! { (String, u32) };

        assert_eq!(
            quote!(#ctx_type).to_string(),
            quote!(#expected_ctx).to_string()
        );
        assert_eq!(
            quote!(#params_type).to_string(),
            quote!(#expected_params).to_string()
        );
    }

    #[test]
    #[should_panic(expected = "RPC handler must take 2 arguments")]
    fn extract_args_types_too_few_args() {
        let input: ItemFn = parse_quote! {
            async fn test_handler(ctx: Context) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
                Ok(serde_json::Value::Null)
            }
        };

        extract_args_types(&input);
    }

    #[test]
    #[should_panic(expected = "RPC handler must take exactly two arguments")]
    fn extract_args_types_too_many_args() {
        let input: ItemFn = parse_quote! {
            async fn test_handler(
                ctx: Context,
                params: String,
                extra: u32,
            ) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
                Ok(serde_json::Value::Null)
            }
        };

        extract_args_types(&input);
    }

    #[test]
    fn macro_expands_correctly() {
        let input = quote! {
            #[rpc(namespace = "test", method = "example")]
            pub async fn example_handler(
                ctx: std::sync::Arc<TestContext>,
                params: String,
            ) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
                Ok(serde_json::json!({"received": params}))
            }
        };

        let ts = syn::parse_str::<ItemFn>(
            &input
                .to_string()
                .replace("#[rpc(namespace = \"test\", method = \"example\")]", ""),
        );
        assert!(ts.is_ok());
    }

    #[test]
    fn namespace_and_method_name_validation() {
        // Test various valid namespace and method combinations
        let test_cases = vec![
            ("simple", "method"),
            ("camelCase", "methodName"),
            ("snake_case", "method_name"),
            ("with123", "numbers456"),
            ("a", "b"), // Single character names
        ];

        for (namespace, method) in test_cases {
            let ts: TokenStream2 = syn::parse_str(&format!(
                "namespace = \"{namespace}\", method = \"{method}\""
            ))
            .unwrap();

            let (ns, m) = parse_attr_tokens_panic(ts);
            assert_eq!(ns, namespace);
            assert_eq!(m, method);
        }
    }

    #[test]
    fn parse_attr_special_characters_in_strings() {
        let ts: TokenStream2 = syn::parse_quote! {
            namespace = "test-ns", method = "method_with_underscores"
        };
        let (ns, m) = parse_attr_tokens_panic(ts);
        assert_eq!(ns, "test-ns");
        assert_eq!(m, "method_with_underscores");
    }

    #[test]
    fn extract_args_with_complex_types() {
        let input: ItemFn = parse_quote! {
            async fn complex_handler(
                ctx: std::sync::Arc<dyn MyTrait + Send + Sync>,
                params: Vec<HashMap<String, serde_json::Value>>,
            ) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
                Ok(serde_json::Value::Null)
            }
        };

        let (ctx_type, params_type) = extract_args_types(&input);

        let expected_ctx: Type = parse_quote! { std::sync::Arc<dyn MyTrait + Send + Sync> };
        let expected_params: Type = parse_quote! { Vec<HashMap<String, serde_json::Value>> };

        assert_eq!(
            quote!(#ctx_type).to_string(),
            quote!(#expected_ctx).to_string()
        );
        assert_eq!(
            quote!(#params_type).to_string(),
            quote!(#expected_params).to_string()
        );
    }

    #[test]
    fn extract_args_with_generic_lifetime() {
        let input: ItemFn = parse_quote! {
            async fn lifetime_handler<'a>(
                ctx: Arc<Context<'a>>,
                params: &'a str,
            ) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
                Ok(serde_json::Value::Null)
            }
        };

        let (ctx_type, params_type) = extract_args_types(&input);

        let expected_ctx: Type = parse_quote! { Arc<Context<'a>> };
        let expected_params: Type = parse_quote! { &'a str };

        assert_eq!(
            quote!(#ctx_type).to_string(),
            quote!(#expected_ctx).to_string()
        );
        assert_eq!(
            quote!(#params_type).to_string(),
            quote!(#expected_params).to_string()
        );
    }

    #[test]
    fn multiple_namespace_method_combinations() {
        let test_cases = vec![
            ("auth", "login", "register_auth_login"),
            ("user", "getProfile", "register_user_getProfile"),
            ("api", "v1_getData", "register_api_v1_getData"),
            ("system", "health_check", "register_system_health_check"),
        ];

        for (namespace, method, expected_fn_name) in test_cases {
            let ts: TokenStream2 = syn::parse_str(&format!(
                "namespace = \"{namespace}\", method = \"{method}\""
            ))
            .unwrap();

            let (ns, m) = parse_attr_tokens_panic(ts);
            assert_eq!(ns, namespace);
            assert_eq!(m, method);

            let register_fn = format_ident!("register_{}_{}", ns, m);
            assert_eq!(register_fn.to_string(), expected_fn_name);
        }
    }

    #[test]
    fn macro_preservation_of_function_attributes() {
        let input: ItemFn = parse_quote! {
            #[doc = "This is a test handler"]
            #[allow(dead_code)]
            pub async fn test_handler(
                ctx: Arc<TestContext>,
                params: String,
            ) -> Result<serde_json::Value, mojave_rpc_core::RpcErr> {
                Ok(serde_json::Value::Null)
            }
        };

        assert!(matches!(input.vis, syn::Visibility::Public(_)));
        assert_eq!(input.attrs.len(), 2);
        assert_eq!(input.sig.ident.to_string(), "test_handler");
    }

    #[test]
    fn params_parsing_handles_all_json_value_types() {
        let params_type: Type = parse_quote! { serde_json::Value };
        let generated = generate_params_parsing(&params_type);

        let generated_str = generated.to_string();

        assert!(generated_str.contains("None =>"));
        assert!(generated_str.contains("0 =>"));
        assert!(generated_str.contains("1 =>"));
        assert!(generated_str.contains("_ =>"));
        assert!(generated_str.contains("mojave_rpc_core :: RpcErr :: BadParams"));
    }

    #[test]
    fn function_signature_validation() {
        let valid_signatures = vec![
            quote! {
                async fn handler(ctx: Arc<C>, params: ()) -> Result<Value, RpcErr>
            },
            quote! {
                pub async fn handler(ctx: C, params: String) -> Result<Value, RpcErr>
            },
            quote! {
                pub(crate) async fn handler(ctx: Box<C>, params: (u32, String)) -> Result<Value, RpcErr>
            },
        ];

        for sig in valid_signatures {
            let input: ItemFn = syn::parse2(quote! {
                #sig {
                    Ok(serde_json::Value::Null)
                }
            })
            .expect("Should parse valid function signature");

            let (_ctx_type, _params_type) = extract_args_types(&input);
        }
    }
}
