use std::time::Duration;

/// Arrival rate
#[derive(Debug, Clone, Copy)]
pub enum Arrival {
    /// Poisson distribution arrival rate (exponential)
    Poisson(f64),
    /// Uniform distribution arrival rate
    Uniform(f64, f64),
    /// Fixed arrival rate
    Fixed(f64),
}

pub struct Config {
    rpc: f64,
    duration: Duration,
    arrival: Arrival,
    burst: usize,
}

pub struct Throttler {
    config: Config,
}
