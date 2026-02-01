#[cfg(test)]
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;
use std::time::Duration;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{AcquireError, Mutex, Semaphore, SemaphorePermit};
use tokio::time::Instant;

/// Error returned when the semaphore is closed.
pub type Error = AcquireError;

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
    /// Maximum tokens in the bucket.
    capacity: f64,
    /// Current available tokens.
    tokens: f64,
    /// Tokens added per second.
    refill_rate: f64,
    /// Last time tokens were refilled.
    last_refill: Instant,
    /// Minimum delay between requests (even with tokens available).
    min_delay: Duration,
    /// Last request time for enforcing min_delay.
    last_request: Option<Instant>,
}

impl RequestLimiter {
    /// Create a new rate limiter with concurrency and rate limits.
    ///
    /// # Arguments
    /// * `max_concurrent` - Maximum concurrent requests (semaphore)
    /// * `requests_per_second` - Maximum requests per second (token bucket)
    pub fn new(max_concurrent: usize, requests_per_second: f64) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            token_bucket: Arc::new(Mutex::new(TokenBucket::new(requests_per_second))),
        }
    }

    /// Acquire a permit to make a request.
    ///
    /// Waits for both concurrency permit and rate limit token.
    /// Returns a permit that releases both when dropped.
    pub async fn acquire(&self) -> Result<RateLimitPermit<'_>, Error> {
        let semaphore_permit = self.semaphore.acquire().await?;

        loop {
            let wait_duration = {
                let mut bucket = self.token_bucket.lock().await;
                bucket.acquire_token()
            };

            match wait_duration {
                Some(duration) if duration > Duration::ZERO => {
                    tokio::time::sleep(duration).await;
                }
                Some(_) => {
                    continue;
                }
                None => break,
            }
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
    #[cfg(test)]
    pub fn calculate_retry_delay(attempt: u32, base_delay_ms: u64, seed: u64) -> Duration {
        let max_delay_ms = base_delay_ms.saturating_mul(2_u64.saturating_pow(attempt));
        let jitter = random_with_seed(max_delay_ms, seed);
        Duration::from_millis(jitter)
    }
}

impl TokenBucket {
    fn new(requests_per_second: f64) -> Self {
        let capacity = requests_per_second * 2.0;
        let min_delay = if requests_per_second.is_finite() && requests_per_second > 0.0 {
            Duration::from_secs_f64(1.0 / requests_per_second)
        } else {
            Duration::ZERO
        };

        Self {
            capacity,
            tokens: capacity,
            refill_rate: requests_per_second,
            last_refill: Instant::now(),
            min_delay,
            last_request: None,
        }
    }

    /// Acquire a token from the bucket.
    ///
    /// Refills tokens based on elapsed time, then returns the
    /// wait duration if no token is available.
    fn acquire_token(&mut self) -> Option<Duration> {
        let now = Instant::now();

        if self.refill_rate.is_finite() && self.refill_rate > 0.0 {
            let elapsed = now.duration_since(self.last_refill);
            let tokens_to_add = elapsed.as_secs_f64() * self.refill_rate;
            self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
        } else if self.refill_rate.is_infinite() {
            self.tokens = self.capacity;
        }

        self.last_refill = now;

        if let Some(last) = self.last_request {
            let time_since_last = now.duration_since(last);
            if time_since_last < self.min_delay {
                return Some(self.min_delay - time_since_last);
            }
        }

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            self.last_request = Some(now);
            return None;
        }

        if self.refill_rate.is_finite() && self.refill_rate > 0.0 {
            let needed = (1.0 - self.tokens).max(0.0);
            let wait_secs = needed / self.refill_rate;
            return Some(Duration::from_secs_f64(wait_secs));
        }

        None
    }
}

/// Permit that releases the semaphore when dropped.
pub struct RateLimitPermit<'a> {
    _semaphore_permit: SemaphorePermit<'a>,
}

/// Generate a random number in [0, max] using a seed.
///
/// Uses hash-based randomization for deterministic but distributed
/// values across different seeds. This ensures concurrent requests
/// with different content get different jitter even when retried
/// at the same time.
#[cfg(test)]
fn random_with_seed(max: u64, seed: u64) -> u64 {
    if max == 0 {
        return 0;
    }

    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(seed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    hasher.write_u64(now.as_nanos() as u64);
    let hash = hasher.finish();
    hash % max.saturating_add(1)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn request_limiter_limits_concurrency() {
        let limiter = RequestLimiter::new(2, 1_000.0);
        let counter = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        for _ in 0..5 {
            let limiter = limiter.clone();
            let counter = Arc::clone(&counter);
            let max_concurrent = Arc::clone(&max_concurrent);

            let handle = tokio::spawn(async move {
                let _permit = limiter
                    .acquire()
                    .await
                    .expect("semaphore should not be closed");

                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                max_concurrent.fetch_max(current, Ordering::SeqCst);

                tokio::time::sleep(Duration::from_millis(10)).await;

                counter.fetch_sub(1, Ordering::SeqCst);
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("task should not panic");
        }

        assert!(max_concurrent.load(Ordering::SeqCst) <= 2);
    }

    #[tokio::test]
    async fn request_limiter_releases_on_drop() {
        let limiter = RequestLimiter::new(1, 1_000.0);

        let _permit1 = limiter
            .acquire()
            .await
            .expect("semaphore should not be closed");

        let mut second = Box::pin(limiter.acquire());
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut second)
                .await
                .is_err(),
            "Second acquire should timeout when concurrency limit is reached"
        );

        drop(_permit1);

        let _permit2 = second
            .await
            .expect("second permit should be available after drop");
    }

    #[tokio::test]
    async fn token_bucket_rate_limits() {
        let limiter = RequestLimiter::new(10, 10.0);

        let start = Instant::now();

        let _permit1 = limiter.acquire().await.expect("should acquire");
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
        tokio::time::pause();

        let mut bucket = TokenBucket::new(0.5);

        assert!(bucket.acquire_token().is_none());

        let wait = bucket
            .acquire_token()
            .expect("bucket should require waiting after token exhaustion");
        assert!(wait >= Duration::from_secs(2));

        tokio::time::advance(Duration::from_secs(2)).await;

        assert!(bucket.acquire_token().is_none());
    }

    #[test]
    fn retry_delay_increases_with_attempt() {
        let seed = 12_345u64;
        let base_delay = 1_000u64;

        let delay0 = RequestLimiter::calculate_retry_delay(0, base_delay, seed);
        let delay1 = RequestLimiter::calculate_retry_delay(1, base_delay, seed);
        let delay2 = RequestLimiter::calculate_retry_delay(2, base_delay, seed);

        assert!(delay0 <= Duration::from_millis(base_delay));
        assert!(delay1 <= Duration::from_millis(base_delay * 2));
        assert!(delay2 <= Duration::from_millis(base_delay * 4));
    }

    #[test]
    fn retry_delay_uses_seed_for_distribution() {
        let base_delay = 1_000u64;

        let delay1 = RequestLimiter::calculate_retry_delay(1, base_delay, 1);
        let delay2 = RequestLimiter::calculate_retry_delay(1, base_delay, 2);
        let delay3 = RequestLimiter::calculate_retry_delay(1, base_delay, 3);

        assert!(delay1 <= Duration::from_millis(2_000));
        assert!(delay2 <= Duration::from_millis(2_000));
        assert!(delay3 <= Duration::from_millis(2_000));

        let delays = [delay1, delay2, delay3];
        let unique_delays: HashSet<_> = delays.iter().collect();
        assert!(
            unique_delays.len() > 1,
            "Expected different delays for different seeds, got {:?}",
            delays
        );
    }

    #[test]
    fn random_with_seed_distribution() {
        let max = 1_000u64;
        let mut values = HashSet::new();

        for seed in 0..100 {
            let value = random_with_seed(max, seed);
            assert!(value <= max, "Value {} exceeds max {}", value, max);
            values.insert(value);
        }

        assert!(
            values.len() >= 50,
            "Expected good distribution, got {} unique values out of 100",
            values.len()
        );
    }
}
