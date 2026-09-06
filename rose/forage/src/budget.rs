//! The global time budget — forage's defining difference from calque.
//!
//! calque bounds capture per page; forage bounds the *whole run*. One deadline
//! drives the scheduler, a politeness gap rate-limits navigations, and a
//! per-action cap stops one hung page from eating the budget.

use std::time::{Duration, Instant};

use tokio::sync::Mutex;

pub struct Budget {
    deadline: Instant,
    /// Minimum gap between navigations (politeness).
    min_gap: Duration,
    /// Hard cap on any single task.
    per_action: Duration,
    last_action: Mutex<Option<Instant>>,
}

impl Budget {
    pub fn new(total: Duration, min_gap: Duration, per_action: Duration) -> Self {
        Self {
            deadline: Instant::now() + total,
            min_gap,
            per_action,
            last_action: Mutex::new(None),
        }
    }

    pub fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    /// The timeout for the next action: the per-action cap, but never longer
    /// than the time left on the global budget.
    pub fn action_timeout(&self) -> Duration {
        self.per_action
            .min(self.remaining())
            .max(Duration::from_millis(1))
    }

    /// Sleep, if needed, to honour the politeness gap since the last action.
    pub async fn throttle(&self) {
        let mut last = self.last_action.lock().await;
        if let Some(prev) = *last {
            let elapsed = prev.elapsed();
            if elapsed < self.min_gap {
                tokio::time::sleep(self.min_gap - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }
}
