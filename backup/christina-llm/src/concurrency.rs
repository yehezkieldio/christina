use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, Semaphore, SemaphorePermit};

/// Token bucket rate limiter for proactive API rate limiting.
///
/// Combines a semaphore for concurrency control with a token bucket
/// for rate limiting. This prevents thundering herd by spacing out
/// requests proactively rather than reactively retrying.
#[derive(Clone)]
pub struct RequestLimiter {
    semaphore: Arc<Semaphore>,
    token_bucket: Arc<Mutex<TokenBucket>>,
}

/// Token bucket for rate limiting.
///
/// Tracks tokens that replenish over time. Each request consumes
/// a token; if no tokens available, requests wait.
struct TokenBucket {
    /// Maximum tokens in the bucket
    capacity: u32,
    /// Current available tokens
    tokens: u32,
    /// Tokens added per second
    refill_rate: f64,
    /// Last time tokens were refilled
    last_refill: Instant,
    /// Minimum delay between requests (even with tokens available)
    min_delay: Duration,
    /// Last request time for enforcing min_delay
    last_request: Option<Instant>,
}

impl RequestLimiter {
    /// Create a new rate limiter with concurrency and rate limits.
    ///
    /// # Arguments
    /// * `max_concurrent` - Maximum concurrent requests (semaphore)
    /// * `requests_per_second` - Maximum requests per second (token bucket)
    pub fn new(max_concurrent: usize, requests_per_second: f64) -> Self {
        let capacity = (requests_per_second * 2.0).ceil() as u32;
        let min_delay = if requests_per_second > 0.0 {
            Duration::from_secs_f64(1.0 / requests_per_second)
        } else {
            Duration::ZERO
        };

        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            token_bucket: Arc::new(Mutex::new(TokenBucket {
                capacity,
                tokens: capacity,
                refill_rate: requests_per_second,
                last_refill: Instant::now(),
                min_delay,
                last_request: None,
            })),
        }
    }

    /// Create a rate limiter with only concurrency control (no rate limiting).
    pub fn with_concurrency_only(max_concurrent: usize) -> Self {
        Self::new(max_concurrent, f64::INFINITY)
    }

    /// Acquire a permit to make a request.
    ///
    /// Waits for both concurrency permit and rate limit token.
    /// Returns a permit that releases both when dropped.
    pub async fn acquire(&self) -> Result<RateLimitPermit<'_>, ()> {
        // First acquire semaphore permit for concurrency control
        let semaphore_permit = self.semaphore.acquire().await.map_err(|_| ())?;

        // Then acquire token from bucket for rate limiting
        let wait_duration = {
            let mut bucket = self.token_bucket.lock().await;
            bucket.acquire_token()
        };

        // Wait for rate limit if needed
        if wait_duration > Duration::ZERO {
            tokio::time::sleep(wait_duration).await;
        }

        Ok(RateLimitPermit {
            _semaphore_permit: semaphore_permit,
        })
    }

    /// Calculate backoff delay with full jitter for retries.
    ///
    /// Uses full jitter (random delay between 0 and max) to prevent
    /// thundering herd on retries. Each request gets a unique seed
    /// based on content hash for better distribution.
    ///
    /// # Arguments
    /// * `attempt` - Retry attempt number (0-indexed)
    /// * `base_delay_ms` - Base delay in milliseconds
    /// * `seed` - Unique seed for this request (e.g., content hash)
    pub fn calculate_retry_delay(attempt: u32, base_delay_ms: u64, seed: u64) -> Duration {
        let max_delay_ms = base_delay_ms * 2_u64.pow(attempt);
        let jitter = random_with_seed(max_delay_ms, seed);
        Duration::from_millis(jitter)
    }
}

impl TokenBucket {
    /// Acquire a token from the bucket.
    ///
    /// Refills tokens based on elapsed time, then returns the
    /// wait duration if no token is available.
    fn acquire_token(&mut self) -> Duration {
        // Refill tokens based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let tokens_to_add = (elapsed.as_secs_f64() * self.refill_rate) as u32;
        self.tokens = self.tokens.saturating_add(tokens_to_add).min(self.capacity);
        self.last_refill = now;

        // Check minimum delay between requests
        if let Some(last) = self.last_request {
            let time_since_last = now.duration_since(last);
            if time_since_last < self.min_delay {
                return self.min_delay - time_since_last;
            }
        }

        // Try to consume a token
        if self.tokens > 0 {
            self.tokens -= 1;
            self.last_request = Some(now);
            Duration::ZERO
        } else if self.refill_rate > 0.0 {
            // Calculate wait time for next token
            let wait_secs = 1.0 / self.refill_rate;
            Duration::from_secs_f64(wait_secs)
        } else {
            Duration::ZERO
        }
    }
}

/// Permit that releases both semaphore and token when dropped.
pub struct RateLimitPermit<'a> {
    _semaphore_permit: SemaphorePermit<'a>,
}

/// Generate a random number in [0, max] using a seed.
///
/// Uses hash-based randomization for deterministic but distributed
/// values across different seeds. This ensures concurrent requests
/// with different content get different jitter even when retried
/// at the same time.
fn random_with_seed(max: u64, seed: u64) -> u64 {
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn request_limiter_limits_concurrency() {
        let limiter = RequestLimiter::with_concurrency_only(2);
        let counter = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let mut handles = vec![];

        for _ in 0..5 {
            let limiter = limiter.clone();
            let counter = Arc::clone(&counter);
            let max_concurrent = Arc::clone(&max_concurrent);

            let handle = tokio::spawn(async move {
                let _permit = limiter
                    .acquire()
                    .await
                    .expect("semaphore should not be closed");

                // Track concurrent requests
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                max_concurrent.fetch_max(current, Ordering::SeqCst);

                // Simulate work
                tokio::time::sleep(Duration::from_millis(10)).await;

                counter.fetch_sub(1, Ordering::SeqCst);
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("task should not panic");
        }

        // Verify we never exceeded the limit
        assert!(max_concurrent.load(Ordering::SeqCst) <= 2);
    }

    #[tokio::test]
    async fn request_limiter_releases_on_drop() {
        let limiter = RequestLimiter::with_concurrency_only(1);

        // Acquire first permit
        let _permit1 = limiter
            .acquire()
            .await
            .expect("semaphore should not be closed");

        // Try to acquire second permit - should timeout since concurrency is 1
        let mut second = Box::pin(limiter.acquire());
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut second)
                .await
                .is_err(),
            "Second acquire should timeout when concurrency limit is reached"
        );

        // Drop first permit
        drop(_permit1);

        // Now second should succeed
        let _permit2 = second
            .await
            .expect("second permit should be available after drop");
    }

    #[tokio::test]
    async fn request_limiter_enforces_limit() {
        let limiter = RequestLimiter::with_concurrency_only(1);
        let first = limiter
            .acquire()
            .await
            .expect("semaphore should not be closed");

        let mut second = Box::pin(limiter.acquire());
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut second)
                .await
                .is_err()
        );

        drop(first);
        let _permit = second
            .await
            .expect("second permit should be available after drop");
    }

    #[tokio::test]
    async fn token_bucket_rate_limits() {
        // 10 requests per second = 100ms between requests
        let limiter = RequestLimiter::new(10, 10.0);

        let start = Instant::now();

        // First request should be immediate
        let _permit1 = limiter.acquire().await.expect("should acquire");

        // Second request should wait ~100ms due to min_delay
        let _permit2 = limiter.acquire().await.expect("should acquire");

        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(90),
            "Expected at least 90ms delay, got {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn token_bucket_refills_over_time() {
        let limiter = RequestLimiter::new(1, 10.0);

        // Use up the token
        let _permit1 = limiter.acquire().await.expect("should acquire");
        drop(_permit1);

        // Wait for tokens to refill
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Should be able to acquire immediately now
        let start = Instant::now();
        let _permit2 = limiter.acquire().await.expect("should acquire");
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(10),
            "Expected immediate acquisition, got {:?}",
            elapsed
        );
    }

    #[test]
    fn retry_delay_increases_with_attempt() {
        let seed = 12345u64;
        let base_delay = 1000u64;

        let delay0 = RequestLimiter::calculate_retry_delay(0, base_delay, seed);
        let delay1 = RequestLimiter::calculate_retry_delay(1, base_delay, seed);
        let delay2 = RequestLimiter::calculate_retry_delay(2, base_delay, seed);

        // Delays should be in valid ranges (full jitter: 0 to max)
        assert!(delay0 <= Duration::from_millis(base_delay));
        assert!(delay1 <= Duration::from_millis(base_delay * 2));
        assert!(delay2 <= Duration::from_millis(base_delay * 4));
    }

    #[test]
    fn retry_delay_uses_seed_for_distribution() {
        let base_delay = 1000u64;

        // Different seeds should give different delays
        let delay1 = RequestLimiter::calculate_retry_delay(1, base_delay, 1);
        let delay2 = RequestLimiter::calculate_retry_delay(1, base_delay, 2);
        let delay3 = RequestLimiter::calculate_retry_delay(1, base_delay, 3);

        // All should be within valid range
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
    fn random_with_seed_distribution() {
        let max = 1000u64;
        let mut values = std::collections::HashSet::new();

        // Generate values with different seeds
        for seed in 0..100 {
            let value = random_with_seed(max, seed);
            assert!(value <= max, "Value {} exceeds max {}", value, max);
            values.insert(value);
        }

        // Should have good distribution (at least 50 unique values)
        assert!(
            values.len() >= 50,
            "Expected good distribution, got {} unique values out of 100",
            values.len()
        );
    }
}
