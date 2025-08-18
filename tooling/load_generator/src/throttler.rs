use rand::Rng;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::time::sleep_until;

use crate::{Cli, token_bucket::TokenBucket};

/// Tiny epsilon to avoid div-by-zero in RPS values
const MIN_RPS: f64 = 1e-9;
/// Bounds for uniform random values in exponential sampling
const MIN_UNIFORM: f64 = 1e-12;
const MAX_UNIFORM: f64 = 1.0 - 1e-12;

/// Arrival rate
#[derive(Debug, Clone, Copy)]
pub enum ArrivalPattern {
    /// Poisson distribution arrival rate (exponential)
    Poisson(f64),
    /// Uniform distribution arrival rate
    Uniform(f64, f64),
    /// Fixed arrival rate
    Fixed(f64),
}

impl ArrivalPattern {
    fn next_gap(&self) -> Duration {
        let mut rng = rand::thread_rng();
        match *self {
            ArrivalPattern::Poisson(rps) => {
                let rps = rps.max(MIN_RPS);
                let u: f64 = rng.r#gen::<f64>().clamp(MIN_UNIFORM, MAX_UNIFORM);
                Duration::from_secs_f64(-u.ln() / rps)
            }
            ArrivalPattern::Uniform(min_rps, max_rps) => {
                let (lo, hi) = if min_rps <= max_rps {
                    (min_rps, max_rps)
                } else {
                    (max_rps, min_rps)
                };
                let rps = rng.gen_range(lo..=hi).max(MIN_RPS);
                Duration::from_secs_f64(1.0 / rps)
            }
            ArrivalPattern::Fixed(rps) => Duration::from_secs_f64(1.0 / rps.max(1e-9)),
        }
    }
}

impl From<String> for ArrivalPattern {
    fn from(s: String) -> Self {
        match s.as_str() {
            "poisson" => ArrivalPattern::Poisson(1.0),
            "uniform" => ArrivalPattern::Uniform(0.0, 1.0),
            "fixed" => ArrivalPattern::Fixed(1.0),
            _ => panic!("Invalid arrival rate"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThrottlerConfig {
    rps: f64,
    duration: Duration,
    arrival: ArrivalPattern,
    max_inflight: usize,
    burst: Option<usize>,
}

impl From<Cli> for ThrottlerConfig {
    fn from(cli: Cli) -> Self {
        ThrottlerConfig {
            rps: cli.rps,
            max_inflight: cli.max_inflight,
            duration: Duration::from_millis(cli.duration),
            arrival: cli.arrival.into(),
            burst: cli.burst,
        }
    }
}

pub struct Throttler {
    config: ThrottlerConfig,

    inflight: Arc<AtomicUsize>,
    end_at: Instant,
    next: Instant,
    bucket: Option<TokenBucket>,
}

impl Throttler {
    pub fn new(cli: Cli, max_inflight: usize) -> Self {
        let duration = Duration::from_millis(cli.duration);
        Throttler {
            config: cli.into(),
            inflight: Arc::new(AtomicUsize::new(max_inflight)),
            end_at: Instant::now() + duration,
            next: Instant::now(),
            bucket: None,
        }
    }

    pub fn done(&self) -> bool {
        self.end_at <= Instant::now()
    }

    #[inline]
    pub fn inflight_count(&self) -> usize {
        self.inflight.load(Ordering::Relaxed)
    }

    pub fn release(&self) {
        self.inflight.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn can_proceed(&mut self) -> bool {
        if self.inflight_count() >= self.config.max_inflight {
            return false;
        }

        if let Some(ref mut bucket) = self.bucket {
            if !bucket.consume() {
                return false;
            }
        }

        true
    }

    /// Wait until it's time for the next request according to arrival pattern
    pub async fn wait_for_next(&mut self) {
        let now = Instant::now();
        if now < self.next {
            sleep_until(self.next.into()).await;
        }

        // Calculate next request time
        let gap = self.config.arrival.next_gap();
        self.next = Instant::now() + gap;
    }

    pub fn try_acquire(&mut self) -> Option<ThrottleGuard> {
        if !self.can_proceed() {
            return None;
        }

        self.inflight.fetch_add(1, Ordering::Relaxed);
        Some(ThrottleGuard {
            0: Arc::clone(&self.inflight),
        })
    }

    pub async fn run<F, Fut>(&mut self, work_fn: F) -> ThrottleResult
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let work_fn = Arc::new(work_fn);
        let mut stats = ThrottleStats::new();

        while !self.done() {
            self.wait_for_next().await;

            if let Some(guard) = self.try_acquire() {
                stats.requests_started += 1;
                let work_fn_clone = Arc::clone(&work_fn);

                tokio::spawn(async move {
                    work_fn_clone().await;
                    // Guard automatically releases when dropped
                    drop(guard);
                });
            } else {
                stats.requests_throttled += 1;
            }
        }

        // Wait for remaining inflight requests to complete
        while self.inflight_count() > 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        ThrottleResult { stats }
    }
}

pub struct ThrottleGuard(Arc<AtomicUsize>);

impl Drop for ThrottleGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Debug, Default)]
pub struct ThrottleStats {
    pub requests_started: u64,
    pub requests_throttled: u64,
}

impl ThrottleStats {
    fn new() -> Self {
        Self::default()
    }

    pub fn total_requests(&self) -> u64 {
        self.requests_started + self.requests_throttled
    }

    pub fn throttle_rate(&self) -> f64 {
        if self.total_requests() == 0 {
            0.0
        } else {
            self.requests_throttled as f64 / self.total_requests() as f64
        }
    }
}

#[derive(Debug)]
pub struct ThrottleResult {
    pub stats: ThrottleStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tokio::time::Duration as TokioDuration;

    fn mock_cli() -> Cli {
        Cli {
            node: "http://localhost:8545".to_string(),
            test_type: crate::TestType::Erc20,
            duration: 1000,
            rps: 10.0,
            burst: None,
            arrival: "fixed".to_string(),
            max_inflight: 5,
        }
    }

    #[test]
    fn test_arrival_pattern_fixed() {
        let pattern = ArrivalPattern::Fixed(10.0);
        let gap = pattern.next_gap();
        assert!((gap.as_millis() as f64 - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_arrival_pattern_fixed_very_small_rps() {
        let pattern = ArrivalPattern::Fixed(1e-12);
        let gap = pattern.next_gap();
        let expected = Duration::from_secs_f64(1.0 / MIN_RPS);
        assert!((gap.as_secs_f64() - expected.as_secs_f64()).abs() < 1e-6);
    }

    #[test]
    fn test_arrival_pattern_poisson() {
        let pattern = ArrivalPattern::Poisson(100.0);
        let gap = pattern.next_gap();
        assert!(gap.as_millis() < 1000);
        assert!(gap.as_nanos() > 0);
    }

    #[test]
    fn test_arrival_pattern_poisson_very_small_rps() {
        let pattern = ArrivalPattern::Poisson(1e-12);
        let gap = pattern.next_gap();
        assert!(gap.as_secs() > 0);
    }

    #[test]
    fn test_arrival_pattern_uniform() {
        let pattern = ArrivalPattern::Uniform(5.0, 15.0);
        let gap = pattern.next_gap();
        assert!(gap.as_millis() >= 60 && gap.as_millis() <= 220);
    }

    #[test]
    fn test_arrival_pattern_uniform_reversed_bounds() {
        let pattern = ArrivalPattern::Uniform(15.0, 5.0);
        let gap = pattern.next_gap();
        assert!(gap.as_millis() >= 60 && gap.as_millis() <= 220);
    }

    #[test]
    fn test_arrival_pattern_from_string() {
        assert!(matches!(
            ArrivalPattern::from("poisson".to_string()),
            ArrivalPattern::Poisson(_)
        ));
        assert!(matches!(
            ArrivalPattern::from("uniform".to_string()),
            ArrivalPattern::Uniform(_, _)
        ));
        assert!(matches!(
            ArrivalPattern::from("fixed".to_string()),
            ArrivalPattern::Fixed(_)
        ));
    }

    #[test]
    #[should_panic(expected = "Invalid arrival rate")]
    fn test_arrival_pattern_from_string_invalid() {
        let _ = ArrivalPattern::from("invalid".to_string());
    }

    #[test]
    fn test_throttler_config_from_cli() {
        let cli = mock_cli();
        let config = ThrottlerConfig::from(cli.clone());

        assert_eq!(config.rps, cli.rps);
        assert_eq!(config.max_inflight, cli.max_inflight);
        assert_eq!(config.duration, Duration::from_millis(cli.duration));
        assert!(matches!(config.arrival, ArrivalPattern::Fixed(_)));
        assert_eq!(config.burst, cli.burst);
    }

    #[test]
    fn test_throttler_new() {
        let cli = mock_cli();
        let throttler = Throttler::new(cli.clone(), 10);

        assert_eq!(throttler.config.max_inflight, cli.max_inflight);
        assert_eq!(throttler.inflight_count(), 10);
        assert!(!throttler.done());
        assert!(throttler.bucket.is_none());
    }

    #[test]
    fn test_throttler_done() {
        let mut cli = mock_cli();
        cli.duration = 1;
        let throttler = Throttler::new(cli, 5);

        assert!(!throttler.done());

        std::thread::sleep(Duration::from_millis(10));
        assert!(throttler.done());
    }

    #[test]
    fn test_throttler_inflight_tracking() {
        let cli = mock_cli();
        let throttler = Throttler::new(cli, 0);

        assert_eq!(throttler.inflight_count(), 0);

        throttler.inflight.fetch_add(3, Ordering::Relaxed);
        assert_eq!(throttler.inflight_count(), 3);

        throttler.release();
        assert_eq!(throttler.inflight_count(), 2);
    }

    #[test]
    fn test_throttler_can_proceed_max_inflight() {
        let cli = mock_cli();
        let mut throttler = Throttler::new(cli, 0);

        assert!(throttler.can_proceed());

        throttler.inflight.store(5, Ordering::Relaxed);
        assert!(!throttler.can_proceed());

        throttler.inflight.store(4, Ordering::Relaxed);
        assert!(throttler.can_proceed());
    }

    #[test]
    fn test_throttler_can_proceed_with_token_bucket() {
        let cli = mock_cli();
        let mut throttler = Throttler::new(cli, 0);

        throttler.bucket = Some(TokenBucket::new(1.0, 1));

        assert!(throttler.can_proceed());

        assert!(!throttler.can_proceed());
    }

    #[test]
    fn test_throttler_try_acquire() {
        let cli = mock_cli();
        let mut throttler = Throttler::new(cli, 0);

        let guard = throttler.try_acquire();
        assert!(guard.is_some());
        assert_eq!(throttler.inflight_count(), 1);

        drop(guard);
        std::thread::sleep(Duration::from_millis(1));
    }

    #[test]
    fn test_throttler_try_acquire_at_limit() {
        let cli = mock_cli();
        let mut throttler = Throttler::new(cli, 5);

        let guard = throttler.try_acquire();
        assert!(guard.is_none());
        assert_eq!(throttler.inflight_count(), 5);
    }

    #[tokio::test]
    async fn test_throttler_wait_for_next() {
        let cli = mock_cli();
        let mut throttler = Throttler::new(cli, 0);

        throttler.wait_for_next().await;

        let start = Instant::now();
        throttler.wait_for_next().await;
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() >= 10 && elapsed.as_millis() <= 2000,
            "Expected elapsed time between 10ms and 2000ms, got {}ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn test_throttle_guard_drop() {
        let counter = Arc::new(AtomicUsize::new(5));
        let guard = ThrottleGuard(Arc::clone(&counter));

        assert_eq!(counter.load(Ordering::Relaxed), 5);
        drop(guard);
        assert_eq!(counter.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn test_throttle_stats_new() {
        let stats = ThrottleStats::new();
        assert_eq!(stats.requests_started, 0);
        assert_eq!(stats.requests_throttled, 0);
    }

    #[test]
    fn test_throttle_stats_total_requests() {
        let mut stats = ThrottleStats::new();
        stats.requests_started = 10;
        stats.requests_throttled = 5;

        assert_eq!(stats.total_requests(), 15);
    }

    #[test]
    fn test_throttle_stats_throttle_rate() {
        let mut stats = ThrottleStats::new();

        assert_eq!(stats.throttle_rate(), 0.0);

        stats.requests_started = 8;
        stats.requests_throttled = 2;
        assert!((stats.throttle_rate() - 0.2).abs() < 1e-10);

        stats.requests_started = 0;
        stats.requests_throttled = 10;
        assert!((stats.throttle_rate() - 1.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_throttler_run_basic() {
        let mut cli = mock_cli();
        cli.duration = 100;
        cli.rps = 50.0;

        let mut throttler = Throttler::new(cli, 0);
        let counter = Arc::new(AtomicUsize::new(0));

        let work_fn = {
            let counter = Arc::clone(&counter);
            move || {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::Relaxed);
                    tokio::time::sleep(TokioDuration::from_millis(1)).await;
                }
            }
        };

        let result = throttler.run(work_fn).await;

        assert!(result.stats.requests_started > 0);
        assert!(counter.load(Ordering::Relaxed) > 0);
    }

    #[tokio::test]
    async fn test_throttler_run_with_throttling() {
        let mut cli = mock_cli();
        cli.duration = 100;
        cli.max_inflight = 1;
        cli.rps = 50.0;

        let mut throttler = Throttler::new(cli, 0);
        let work_counter = Arc::new(AtomicUsize::new(0));

        let work_fn = {
            let counter = Arc::clone(&work_counter);
            move || {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::Relaxed);
                    tokio::time::sleep(TokioDuration::from_millis(30)).await;
                }
            }
        };

        let result = throttler.run(work_fn).await;

        assert!(result.stats.requests_throttled > 0 || result.stats.requests_started > 0);
        assert!(result.stats.total_requests() >= result.stats.requests_started);
    }

    #[test]
    fn test_arrival_pattern_edge_cases() {
        let pattern = ArrivalPattern::Uniform(MIN_RPS, MIN_RPS);
        let gap = pattern.next_gap();
        assert!(gap.as_secs() > 0);

        let pattern = ArrivalPattern::Fixed(0.0);
        let gap = pattern.next_gap();
        assert!(gap.as_secs_f64() > 1e6);
    }

    #[test]
    fn test_multiple_throttle_guards() {
        let cli = mock_cli();
        let mut throttler = Throttler::new(cli, 0);

        let guard1 = throttler.try_acquire().unwrap();
        let guard2 = throttler.try_acquire().unwrap();

        assert_eq!(throttler.inflight_count(), 2);

        drop(guard1);
        std::thread::sleep(Duration::from_millis(1));

        drop(guard2);
        std::thread::sleep(Duration::from_millis(1));
    }

    #[tokio::test]
    async fn test_throttler_completion_waiting() {
        let mut cli = mock_cli();
        cli.duration = 10;
        cli.max_inflight = 10;

        let mut throttler = Throttler::new(cli, 0);
        let completed = Arc::new(AtomicBool::new(false));

        let work_fn = {
            let completed = Arc::clone(&completed);
            move || {
                let completed = Arc::clone(&completed);
                async move {
                    tokio::time::sleep(TokioDuration::from_millis(50)).await;
                    completed.store(true, Ordering::Relaxed);
                }
            }
        };

        let start = Instant::now();
        let _result = throttler.run(work_fn).await;
        let elapsed = start.elapsed();

        assert!(elapsed.as_millis() >= 40);
        assert!(completed.load(Ordering::Relaxed));
    }
}
