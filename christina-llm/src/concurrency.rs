use std::sync::Arc;

use tokio::sync::{Semaphore, SemaphorePermit};

#[derive(Debug)]
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
