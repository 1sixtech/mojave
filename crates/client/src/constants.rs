use std::time::Duration;

pub(crate) const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(100);
pub(crate) const BACKOFF_FACTOR: u32 = 2;
pub(crate) const MAX_DELAY: Duration = Duration::from_secs(30);
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const DEFAULT_MAX_RETRY: usize = 1;
