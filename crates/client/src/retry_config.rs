use std::time::Duration;

use crate::constants::{BACKOFF_FACTOR, DEFAULT_MAX_RETRY, INITIAL_RETRY_DELAY, MAX_DELAY};

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: usize,
    pub initial_delay: Duration,
    pub backoff_factor: u32,
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRY,
            initial_delay: INITIAL_RETRY_DELAY,
            backoff_factor: BACKOFF_FACTOR,
            max_delay: MAX_DELAY,
        }
    }
}
