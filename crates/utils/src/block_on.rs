pub fn block_on_current_thread<F, Fut, T, E>(proc: F) -> Result<T, E>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    T: serde::Serialize,
    E: std::error::Error + 'static,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move { proc().await })
}
