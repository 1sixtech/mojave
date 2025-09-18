use anyhow::{Error, Result};
use tokio::runtime::Builder;

pub fn run_on_tokio_single<F, Fut, T>(proc: F) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, Error>>,
{
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::msg(format!("Failed to build single-threaded runtime: {e}")))?;
    rt.block_on(proc())
}

pub fn run_on_tokio_multi<F, Fut, T>(proc: F) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, Error>>,
{
    let rt = Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::msg(format!("Failed to build multi-threaded runtime: {e}")))?;
    rt.block_on(proc())
}
