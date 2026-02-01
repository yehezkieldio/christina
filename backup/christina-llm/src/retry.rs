use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::Duration;

use tokio::time::sleep;

/// Retry policy configuration.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Base delay in milliseconds for exponential backoff.
    pub base_delay_ms: u64,
    /// Whether to use full jitter (0 to max) instead of additive jitter.
    pub with_jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
            with_jitter: true,
        }
    }
}

impl RetryPolicy {
    #[cfg(test)]
    pub fn new(max_retries: u32, base_delay_ms: u64) -> Self {
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
    /// Uses full jitter (random delay between 0 and max) to prevent
    /// thundering herd. Without jitter, returns deterministic exponential delay.
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let max_delay_ms = self.base_delay_ms * 2_u64.pow(attempt);
        let delay_ms = if self.with_jitter {
            // Full jitter: random delay between 0 and max
            rand_jitter(max_delay_ms)
        } else {
            max_delay_ms
        };
        Duration::from_millis(delay_ms)
    }

    /// Calculate delay with a specific seed for deterministic jitter.
    ///
    /// This is useful when you want different delays for concurrent requests
    /// that would otherwise retry at the same time.
    pub fn calculate_delay_with_seed(&self, attempt: u32, seed: u64) -> Duration {
        let max_delay_ms = self.base_delay_ms * 2_u64.pow(attempt);
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
///
/// The operation is retried if:
/// - It fails with a transient error
/// - Maximum retries have not been reached
pub async fn retry_with_backoff<F, Fut, T, E>(
    policy: &RetryPolicy,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: IsTransient,
{
    let mut last_error = None;

    for attempt in 0..=policy.max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                // Check if error is transient
                if !err.is_transient() {
                    return Err(err);
                }

                // If this was the last attempt, return the error
                if attempt >= policy.max_retries {
                    return Err(err);
                }

                // Store error and calculate backoff delay
                last_error = Some(err);
                let delay = policy.calculate_delay(attempt);
                sleep(delay).await;
            }
        }
    }

    // This should never happen due to the loop logic, but satisfy the compiler
    #[expect(
        clippy::expect_used,
        reason = "last_error is guaranteed to be Some due to loop logic"
    )]
    Err(last_error.expect("retry loop should have at least one error"))
}

/// Generate random jitter in range [0, max] using current time.
fn rand_jitter(max: u64) -> u64 {
    rand_jitter_with_seed(max, 0)
}

/// Generate random jitter in range [0, max] using a seed for distribution.
///
/// Different seeds produce different jitter values even when called
/// at the same time, preventing thundering herd for concurrent requests.
fn rand_jitter_with_seed(max: u64, seed: u64) -> u64 {
    if max == 0 {
        return 0;
    }

    let random_state = RandomState::new();
    let mut hasher = random_state.build_hasher();
    hasher.write_u64(seed);
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
    );
    let hash = hasher.finish();
    hash % (max + 1)
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test code expects are acceptable for clarity"
)]
mod tests {
    use super::*;
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
    async fn retry_succeeds_after_failures_fast() {
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
        assert_eq!(*attempts.lock().expect("mutex should not be poisoned"), 1); // Should not retry non-transient errors
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
        assert_eq!(*attempts.lock().expect("mutex should not be poisoned"), 3); // 1 initial + 2 retries
    }

    #[test]
    fn delay_calculation_exponential() {
        let policy = RetryPolicy::new(3, 1000).without_jitter();

        assert_eq!(policy.calculate_delay(0), Duration::from_millis(1000));
        assert_eq!(policy.calculate_delay(1), Duration::from_millis(2000));
        assert_eq!(policy.calculate_delay(2), Duration::from_millis(4000));
    }

    #[tokio::test]
    async fn retry_transient_error() {
        let policy = RetryPolicy::new(1, 10).without_jitter();
        let attempts = Arc::new(Mutex::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result = retry_with_backoff(&policy, move || {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                let mut count = attempts.lock().expect("mutex should not be poisoned");
                *count += 1;
                if *count == 1 {
                    Err::<i32, _>(TestError { transient: true })
                } else {
                    Ok(7)
                }
            }
        })
        .await;

        assert_eq!(result.expect("should succeed after transient error"), 7);
        assert_eq!(*attempts.lock().expect("mutex should not be poisoned"), 2);
    }

    #[tokio::test]
    async fn retry_permanent_error_fails_fast() {
        let policy = RetryPolicy::new(5, 10).without_jitter();
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
    async fn retry_max_attempts_exceeded() {
        let policy = RetryPolicy::new(1, 5).without_jitter();
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
        assert_eq!(*attempts.lock().expect("mutex should not be poisoned"), 2);
    }

    #[tokio::test]
    async fn retry_backoff_timing() {
        let policy = RetryPolicy::new(1, 5).without_jitter();
        let attempts = Arc::new(Mutex::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let start = std::time::Instant::now();
        let _ = retry_with_backoff(&policy, move || {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                *attempts.lock().expect("mutex should not be poisoned") += 1;
                Err::<i32, _>(TestError { transient: true })
            }
        })
        .await;

        assert!(start.elapsed() >= Duration::from_millis(5));
    }

    #[test]
    fn delay_with_seed_produces_different_values() {
        let policy = RetryPolicy::default();
        let base_seed = 12345u64;

        // Different seeds should produce different delays
        let delay1 = policy.calculate_delay_with_seed(1, base_seed);
        let delay2 = policy.calculate_delay_with_seed(1, base_seed + 1);
        let delay3 = policy.calculate_delay_with_seed(1, base_seed + 2);

        // All should be within valid range (0 to 2000ms for attempt 1)
        assert!(delay1 <= Duration::from_millis(2000));
        assert!(delay2 <= Duration::from_millis(2000));
        assert!(delay3 <= Duration::from_millis(2000));

        // With full jitter and different seeds, delays should differ
        // (not guaranteed but highly probable)
        let delays = [delay1, delay2, delay3];
        let unique_delays: std::collections::HashSet<_> = delays.iter().collect();
        assert!(
            unique_delays.len() > 1,
            "Expected different delays for different seeds, got {:?}",
            delays
        );
    }

    #[test]
    fn full_jitter_range() {
        let policy = RetryPolicy::default();
        let mut seen_values = std::collections::HashSet::new();

        // Collect multiple samples to verify distribution
        for i in 0..100 {
            let delay = policy.calculate_delay_with_seed(0, i);
            seen_values.insert(delay.as_millis() as u64);
            assert!(
                delay <= Duration::from_millis(1000),
                "Delay {:?} exceeds max 1000ms",
                delay
            );
        }

        // Should have good distribution (at least 50 unique values)
        assert!(
            seen_values.len() >= 50,
            "Expected good distribution, got {} unique values out of 100",
            seen_values.len()
        );
    }

}
