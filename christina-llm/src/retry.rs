use std::time::Duration;

use tokio::time::sleep;

/// Retry policy configuration.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Base delay in milliseconds for exponential backoff.
    pub base_delay_ms: u64,
    /// Whether to add random jitter to avoid thundering herd.
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
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let base = self.base_delay_ms * 2_u64.pow(attempt);
        let delay_ms = if self.with_jitter {
            base + rand_jitter(base)
        } else {
            base
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

fn rand_jitter(max: u64) -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let random_state = RandomState::new();
    let mut hasher = random_state.build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
    );
    let hash = hasher.finish();
    hash % (max + 1)
}
