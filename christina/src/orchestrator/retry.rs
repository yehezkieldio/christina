//! Exponential backoff retry logic with full jitter.
//!
//! WHY exponential backoff: Prevents thundering herd after transient failures (e.g., API outage).
//! Linear backoff (1s, 2s, 3s) still causes synchronized retries; exponential (1s, 2s, 4s, 8s)
//! spreads load over time as failed requests exponentially diverge.
//!
//! WHY full jitter: Randomizes delay in [0, max] instead of fixed exponential. When N requests
//! fail simultaneously (e.g., rate limit), they retry at different times, preventing synchronized
//! storms. Without jitter, all requests retry at exactly 1s, 2s, 4s—defeating backoff purpose.
//!
//! WHY IsTransient trait bound: Type-safe retry classification. Only errors marked transient
//! (rate limits, timeouts, server errors) are retried. Permanent errors (auth failures, invalid
//! requests) fail fast. Alternative (runtime check) would be error-prone and harder to verify.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use christina_core::error::{CompletionError, IsTransient};
use tokio::time::sleep;

/// Retry policy configuration.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts.
    pub max_retries: usize,
    /// Base delay in milliseconds for exponential backoff.
    pub base_delay_ms: u64,
    /// Whether to use full jitter (0 to max) instead of deterministic delay.
    pub with_jitter: bool,
}

pub trait RetryAfter {
    fn retry_after(&self) -> Option<Duration>;
}

impl RetryAfter for CompletionError {
    fn retry_after(&self) -> Option<Duration> {
        CompletionError::retry_after(self)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1_000,
            with_jitter: true,
        }
    }
}

impl RetryPolicy {
    #[cfg(test)]
    pub fn new(max_retries: usize, base_delay_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms,
            with_jitter: true,
        }
    }

    #[cfg(test)]
    pub fn without_jitter(mut self) -> Self {
        self.with_jitter = false;
        self
    }

    /// Calculate delay for a given retry attempt (0-indexed).
    ///
    /// WHY saturating arithmetic: Prevents overflow on large attempt numbers.
    /// 2^32 * base_delay would overflow u64; saturating_pow/saturating_mul cap at u64::MAX,
    /// producing very long (but finite) delays instead of panicking.
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let max_delay_ms = self
            .base_delay_ms
            .saturating_mul(2_u64.saturating_pow(attempt));
        let delay_ms = if self.with_jitter {
            rand_jitter_with_seed(max_delay_ms, 0)
        } else {
            max_delay_ms
        };
        Duration::from_millis(delay_ms)
    }

    /// Calculate delay with a specific seed for deterministic jitter.
    #[cfg(test)]
    pub fn calculate_delay_with_seed(&self, attempt: u32, seed: u64) -> Duration {
        let max_delay_ms = self
            .base_delay_ms
            .saturating_mul(2_u64.saturating_pow(attempt));
        let delay_ms = if self.with_jitter {
            rand_jitter_with_seed(max_delay_ms, seed)
        } else {
            max_delay_ms
        };
        Duration::from_millis(delay_ms)
    }
}

/// Retry a fallible operation with exponential backoff.
///
/// WHY loop instead of recursion: Rust doesn't optimize tail recursion. Recursive implementation
/// would blow stack after ~1000 retries. Loop is stack-safe and clearer for imperative retry logic.
///
/// WHY check transient first: Fail fast on permanent errors (auth, validation). Avoids wasting
/// time/delays on errors that will never succeed. Transient check is cheap (enum match).
pub async fn retry_with_backoff<F, Fut, T, E>(policy: &RetryPolicy, operation: F) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: IsTransient + RetryAfter,
{
    let mut attempt = 0usize;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                if !err.is_transient() {
                    return Err(err);
                }

                if attempt >= policy.max_retries {
                    return Err(err);
                }

                let backoff = policy.calculate_delay(attempt as u32);
                let delay = err.retry_after().map_or(backoff, |retry_after| {
                    std::cmp::min(retry_after, backoff)
                });
                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// Generate random jitter in range [0, max] using a seed for distribution.
///
/// WHY seed + time: Seed alone would produce identical jitter for same content retried
/// simultaneously. Time (nanoseconds) adds entropy, ensuring different jitter even for
/// duplicate requests. Combines determinism (seed) with randomness (time).
///
/// WHY hash-based: Avoids `rand` crate dependency for simple jitter. Hash distribution
/// is sufficient for retry timing (doesn't need cryptographic quality). Fast and simple.
pub fn rand_jitter_with_seed(max: u64, seed: u64) -> u64 {
    if max == 0 {
        return 0;
    }

    // Special-case u64::MAX to avoid overflow when adding 1
    if max == u64::MAX {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(seed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        hasher.write_u64(now.as_nanos() as u64);
        return hasher.finish();
    }

    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(seed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    hasher.write_u64(now.as_nanos() as u64);
    let hash = hasher.finish();
    hash % (max + 1)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct TestError {
        transient: bool,
    }

    impl IsTransient for TestError {
        fn is_transient(&self) -> bool {
            self.transient
        }
    }

    impl RetryAfter for TestError {
        fn retry_after(&self) -> Option<Duration> {
            None
        }
    }

    #[tokio::test]
    async fn retry_succeeds_immediately() {
        let policy = RetryPolicy::default().without_jitter();
        let attempts = Arc::new(Mutex::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result = retry_with_backoff(&policy, move || {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                *attempts.lock().expect("mutex should not be poisoned") += 1;
                Ok::<_, TestError>(42)
            }
        })
        .await;

        assert_eq!(result.expect("should succeed"), 42);
        assert_eq!(*attempts.lock().expect("mutex should not be poisoned"), 1);
    }

    #[tokio::test]
    async fn retry_succeeds_after_failures() {
        tokio::time::pause();

        let policy = RetryPolicy::default().without_jitter();
        let attempts = Arc::new(Mutex::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let start = tokio::time::Instant::now();

        let result = retry_with_backoff(&policy, move || {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                let mut count = attempts.lock().expect("mutex should not be poisoned");
                *count += 1;
                let current = *count;
                drop(count);

                if current < 3 {
                    Err(TestError { transient: true })
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.expect("should succeed"), 42);
        assert!(start.elapsed() >= Duration::from_secs(3));
    }

    #[tokio::test]
    async fn retry_fails_on_non_transient() {
        let policy = RetryPolicy::default().without_jitter();
        let attempts = Arc::new(Mutex::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result = retry_with_backoff(&policy, move || {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                *attempts.lock().expect("mutex should not be poisoned") += 1;
                Err::<i32, _>(TestError { transient: false })
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(*attempts.lock().expect("mutex should not be poisoned"), 1);
    }

    #[tokio::test]
    async fn retry_exhausts_attempts() {
        let policy = RetryPolicy::new(2, 100).without_jitter();
        let attempts = Arc::new(Mutex::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result = retry_with_backoff(&policy, move || {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                *attempts.lock().expect("mutex should not be poisoned") += 1;
                Err::<i32, _>(TestError { transient: true })
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(*attempts.lock().expect("mutex should not be poisoned"), 3);
    }

    #[test]
    fn delay_calculation_exponential() {
        let policy = RetryPolicy::new(3, 1_000).without_jitter();

        assert_eq!(policy.calculate_delay(0), Duration::from_millis(1_000));
        assert_eq!(policy.calculate_delay(1), Duration::from_millis(2_000));
        assert_eq!(policy.calculate_delay(2), Duration::from_millis(4_000));
    }

    #[test]
    fn full_jitter_range() {
        let policy = RetryPolicy::default();
        let mut seen_values = HashSet::new();

        for i in 0..100 {
            let delay = policy.calculate_delay_with_seed(0, i);
            seen_values.insert(delay.as_millis() as u64);
            assert!(
                delay <= Duration::from_millis(1_000),
                "Delay {:?} exceeds max 1000ms",
                delay
            );
        }

        assert!(
            seen_values.len() >= 50,
            "Expected good distribution, got {} unique values out of 100",
            seen_values.len()
        );
    }
}
