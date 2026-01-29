use std::sync::Arc;

use tokio::sync::{Semaphore, SemaphorePermit};

#[derive(Clone)]
pub struct RequestLimiter {
    semaphore: Arc<Semaphore>,
}

impl RequestLimiter {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    pub async fn acquire(&self) -> Result<SemaphorePermit<'_>, ()> {
        self.semaphore.acquire().await.map_err(|_| ())
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test code expects are acceptable for clarity"
)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn request_limiter_limits_concurrency() {
        let limiter = RequestLimiter::new(2);
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
                sleep(Duration::from_millis(10)).await;

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
        let limiter = RequestLimiter::new(1);

        {
            let _permit = limiter
                .acquire()
                .await
                .expect("semaphore should not be closed");
            assert_eq!(limiter.semaphore.available_permits(), 0);
        } // permit dropped here

        // Should be available again
        assert_eq!(limiter.semaphore.available_permits(), 1);
    }

    #[tokio::test]
    async fn request_limiter_enforces_limit() {
        let limiter = RequestLimiter::new(1);
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
    async fn request_limiter_concurrent_requests() {
        let limiter = RequestLimiter::new(2);
        let counter = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..4 {
            let limiter = limiter.clone();
            let counter = Arc::clone(&counter);
            handles.push(tokio::spawn(async move {
                let _permit = limiter
                    .acquire()
                    .await
                    .expect("semaphore should not be closed");
                counter.fetch_add(1, Ordering::SeqCst);
                sleep(Duration::from_millis(5)).await;
                counter.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for handle in handles {
            handle.await.expect("task should not panic");
        }

        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}
