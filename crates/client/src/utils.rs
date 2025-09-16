use std::pin::Pin;

use ethrex_rpc::{
    clients::eth::RpcResponse,
    utils::{RpcRequest, RpcRequestId},
};
use futures::{
    FutureExt,
    future::{Fuse, select_ok},
};
use mojave_utils::rpc::types::MojaveRequestMethods;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde_json::to_string;

use crate::{
    error::{Error, Result},
    retry_config::RetryConfig,
};

pub fn parse_urls(urls: Vec<String>) -> Result<Vec<Url>> {
    urls.into_iter()
        .map(|url| Url::parse(&url).map_err(|e| Error::Custom(e.to_string())))
        .collect()
}

pub fn create_rpc_request(
    method: MojaveRequestMethods,
    params: Option<Vec<serde_json::Value>>,
) -> Result<RpcRequest> {
    Ok(RpcRequest {
        id: RpcRequestId::Number(1),
        jsonrpc: "2.0".to_string(),
        method: to_string(&method)?,
        params,
    })
}

pub fn is_retryable_error(error: &Error) -> bool {
    matches!(error, Error::TimeOut)
}

pub async fn send_request_sequential<T>(
    client: &reqwest::Client,
    request: &RpcRequest,
    urls: &[Url],
    retry_config: &RetryConfig,
) -> Result<T>
where
    T: DeserializeOwned,
{
    let mut last_error = Error::Custom("All RPC calls failed".to_owned());

    for url in urls {
        match send_request_with_retry(client, request, url, retry_config).await {
            Ok(response) => return Ok(response),
            Err(error) => last_error = error,
        }
    }

    Err(last_error)
}

pub async fn send_request_race<T>(
    client: &reqwest::Client,
    request: &RpcRequest,
    urls: &[Url],
) -> Result<T>
where
    T: DeserializeOwned,
{
    let requests: Vec<Pin<Box<Fuse<_>>>> = urls
        .iter()
        .map(|url| Box::pin(send_request_once(client, request, url).fuse()))
        .collect();

    let (response, _) = select_ok(requests)
        .await
        .map_err(|error| Error::Custom(format!("All RPC calls failed: {error}")))?;

    Ok(response)
}

pub async fn send_request_with_retry<T>(
    client: &reqwest::Client,
    request: &RpcRequest,
    url: &Url,
    retry_config: &RetryConfig,
) -> Result<T>
where
    T: DeserializeOwned,
{
    let mut attempt = 0;
    let mut delay = retry_config.initial_delay;
    let mut last_error = None;

    while attempt < retry_config.max_retries {
        attempt += 1;

        match send_request_once(client, request, url).await {
            Ok(response) => return Ok(response),
            Err(error) => {
                tracing::error!(
                    error = %error,
                    attempt = attempt,
                    max_retries = retry_config.max_retries,
                    "Request failed"
                );

                if is_retryable_error(&error) && attempt < retry_config.max_retries {
                    tracing::info!(
                        delay = ?delay,
                        attempt = attempt,
                        max_retries = retry_config.max_retries,
                        "Retrying request"
                    );
                    tokio::time::sleep(delay).await;

                    delay = delay.saturating_mul(retry_config.backoff_factor);
                    if delay > retry_config.max_delay {
                        delay = retry_config.max_delay;
                    }

                    last_error = Some(error);
                } else {
                    return Err(error);
                }
            }
        }
    }

    Err(last_error.unwrap_or(Error::RetryFailed(retry_config.max_retries as u64)))
}

pub async fn send_request_once<T>(
    client: &reqwest::Client,
    request: &RpcRequest,
    url: &Url,
) -> Result<T>
where
    T: DeserializeOwned,
{
    let response = client
        .post(url.as_ref())
        .header("content-type", "application/json")
        .body(serde_json::to_string(request)?)
        .send()
        .await?
        .json::<RpcResponse>()
        .await?;

    match response {
        RpcResponse::Success(ok_response) => Ok(serde_json::from_value::<T>(ok_response.result)?),
        RpcResponse::Error(error_response) => Err(Error::Custom(format!(
            "RPC Error {}: {}",
            error_response.error.code, error_response.error.message
        ))),
    }
}
