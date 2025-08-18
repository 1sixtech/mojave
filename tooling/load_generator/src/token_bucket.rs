use std::time::Instant;

pub struct TokenBucket {
    rps: f64,
    capacity: usize,
    tokens: f64,
    last: Instant,
}

impl TokenBucket {
    pub fn new(rps: f64, capacity: usize) -> Self {
        TokenBucket {
            rps,
            capacity,
            tokens: capacity as f64,
            last: Instant::now(),
        }
    }

    pub fn consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_millis() as f64 / 1000.0;
        self.tokens += self.rps * elapsed;
        self.tokens = self.tokens.min(self.capacity as f64);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn test_new_token_bucket() {
        let bucket = TokenBucket::new(10.0, 5);
        assert_eq!(bucket.rps, 10.0);
        assert_eq!(bucket.capacity, 5);
        assert_eq!(bucket.tokens, 5.0);
    }

    #[test]
    fn test_consume_with_full_bucket() {
        let mut bucket = TokenBucket::new(10.0, 5);
        assert!(bucket.consume());
        assert_eq!(bucket.tokens, 4.0);
    }

    #[test]
    fn test_consume_multiple_tokens() {
        let mut bucket = TokenBucket::new(10.0, 5);
        for i in 0..5 {
            assert!(bucket.consume(), "Failed to consume token {}", i + 1);
        }
        assert!(!bucket.consume());
    }

    #[test]
    fn test_consume_with_empty_bucket() {
        let mut bucket = TokenBucket::new(10.0, 1);
        assert!(bucket.consume());
        assert!(!bucket.consume());
    }

    #[test]
    fn test_token_refill_over_time() {
        let mut bucket = TokenBucket::new(1000.0, 1); // 1000 tokens per second
        assert!(bucket.consume());
        assert!(!bucket.consume());

        thread::sleep(Duration::from_millis(10));

        assert!(bucket.consume());
    }

    #[test]
    fn test_capacity_limit() {
        let mut bucket = TokenBucket::new(1000.0, 2);
        assert!(bucket.consume());
        assert!(bucket.consume());
        assert!(!bucket.consume());

        thread::sleep(Duration::from_millis(10));

        assert!(bucket.consume());
        assert!(bucket.consume());
        assert!(!bucket.consume());
    }

    #[test]
    fn test_zero_rps() {
        let mut bucket = TokenBucket::new(0.0, 10);
        for _ in 0..10 {
            assert!(bucket.consume());
        }

        thread::sleep(Duration::from_millis(100));
        assert!(!bucket.consume());
    }

    #[test]
    fn test_high_rps() {
        let mut bucket = TokenBucket::new(10000.0, 100);
        for _ in 0..100 {
            assert!(bucket.consume());
        }

        thread::sleep(Duration::from_millis(1));
        assert!(bucket.consume());
    }

    #[test]
    fn test_single_capacity_bucket() {
        let mut bucket = TokenBucket::new(100.0, 1);
        assert!(bucket.consume());
        assert!(!bucket.consume());

        thread::sleep(Duration::from_millis(20));
        assert!(bucket.consume());
        assert!(!bucket.consume());
    }

    #[test]
    fn test_large_capacity_bucket() {
        let mut bucket = TokenBucket::new(1.0, 1000);
        for i in 0..1000 {
            assert!(bucket.consume(), "Failed at token {i}");
        }
        assert!(!bucket.consume());
    }

    #[test]
    fn test_time_precision() {
        let mut bucket = TokenBucket::new(1000.0, 1);
        assert!(bucket.consume());

        thread::sleep(Duration::from_nanos(1_000_000));

        assert!(bucket.consume());
    }

    #[test]
    fn test_fractional_tokens() {
        let mut bucket = TokenBucket::new(1.5, 3);
        for _ in 0..3 {
            assert!(bucket.consume());
        }
        assert!(!bucket.consume());

        thread::sleep(Duration::from_millis(700)); // 0.7 seconds * 1.5 = 1.05 tokens
        assert!(bucket.consume());
        assert!(!bucket.consume());
    }

    #[test]
    fn test_burst_behavior() {
        let mut bucket = TokenBucket::new(10.0, 5);
        for _ in 0..5 {
            assert!(bucket.consume());
        }
        assert!(!bucket.consume());

        thread::sleep(Duration::from_millis(200)); // 0.2s * 10 = 2 tokens
        assert!(bucket.consume());
        assert!(bucket.consume());
        assert!(!bucket.consume());
    }

    #[test]
    fn test_edge_case_very_small_rps() {
        let mut bucket = TokenBucket::new(0.1, 1);
        assert!(bucket.consume());
        assert!(!bucket.consume());

        thread::sleep(Duration::from_millis(100));
        assert!(!bucket.consume());

        thread::sleep(Duration::from_millis(10000));
        assert!(bucket.consume());
    }

    #[test]
    fn test_multiple_consume_calls_in_sequence() {
        let mut bucket = TokenBucket::new(2.0, 1);
        assert!(bucket.consume());

        for _ in 0..10 {
            assert!(!bucket.consume());
        }

        thread::sleep(Duration::from_millis(500));
        assert!(bucket.consume());
    }

    #[test]
    fn test_bucket_with_zero_capacity() {
        let mut bucket = TokenBucket::new(10.0, 0);
        assert!(!bucket.consume());

        thread::sleep(Duration::from_millis(100));
        assert!(!bucket.consume());
    }

    #[test]
    fn test_token_accumulation_doesnt_exceed_capacity() {
        let mut bucket = TokenBucket::new(100.0, 3);
        assert!(bucket.consume());
        assert!(bucket.consume());

        thread::sleep(Duration::from_millis(100));

        assert!(bucket.consume());
        assert!(bucket.consume());
        assert!(bucket.consume());
        assert!(!bucket.consume());
    }
}
