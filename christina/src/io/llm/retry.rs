#![allow(
    dead_code,
    reason = "retry policy is defined here but wired into providers later"
)]

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

pub trait IsTransient {
    fn is_transient(&self) -> bool;
}

/// Retry a fallible operation with exponential backoff.
pub async fn retry_with_backoff<F, Fut, T, E>(policy: &RetryPolicy, operation: F) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: IsTransient,
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

                let delay = policy.calculate_delay(attempt as u32);
                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// Generate random jitter in range [0, max] using a seed for distribution.
pub fn rand_jitter_with_seed(max: u64, seed: u64) -> u64 {
    if max == 0 {
        return 0;
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
