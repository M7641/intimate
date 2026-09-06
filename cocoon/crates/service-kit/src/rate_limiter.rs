use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Fixed-window rate limiter backed by an atomic counter.
///
/// A background tokio task resets the counter at the start of each window.
/// `try_acquire()` is lock-free and safe to call from async middleware.
#[derive(Clone)]
pub struct RateLimiter {
    counter: Arc<AtomicU64>,
    max_requests: u64,
}

impl RateLimiter {
    /// Create a rate limiter and spawn a background reset task.
    ///
    /// - `max_requests`: maximum allowed requests per window
    /// - `window`: duration of each fixed window
    pub fn new(max_requests: u64, window: Duration) -> Self {
        let counter = Arc::new(AtomicU64::new(0));

        let counter_bg = counter.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(window).await;
                counter_bg.store(0, Ordering::Relaxed);
            }
        });

        Self {
            counter,
            max_requests,
        }
    }

    /// Try to acquire a request slot. Returns `false` if the limit is exceeded.
    pub fn try_acquire(&self) -> bool {
        let prev = self.counter.fetch_add(1, Ordering::Relaxed);
        prev < self.max_requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn allows_up_to_limit() {
        let rl = RateLimiter::new(5, Duration::from_secs(60));
        for _ in 0..5 {
            assert!(rl.try_acquire());
        }
        assert!(!rl.try_acquire()); // 6th should fail
    }

    #[tokio::test]
    async fn resets_after_window() {
        let rl = RateLimiter::new(2, Duration::from_millis(50));
        assert!(rl.try_acquire());
        assert!(rl.try_acquire());
        assert!(!rl.try_acquire());

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(rl.try_acquire()); // counter reset
    }

    #[tokio::test]
    async fn zero_limit_rejects_all() {
        let rl = RateLimiter::new(0, Duration::from_secs(60));
        assert!(!rl.try_acquire());
    }
}
