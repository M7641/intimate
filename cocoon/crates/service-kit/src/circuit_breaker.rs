use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::AppError;

/// Circuit breaker states.
const CLOSED: u32 = 0;
const OPEN: u32 = 1;
const HALF_OPEN: u32 = 2;

/// Lock-free circuit breaker protecting a downstream dependency.
///
/// State machine:
/// ```text
/// CLOSED --(N consecutive failures)--> OPEN --(cooldown)--> HALF_OPEN
/// HALF_OPEN --(probe success)--> CLOSED
/// HALF_OPEN --(probe failure)--> OPEN (reset cooldown)
/// ```
#[derive(Clone)]
pub struct CircuitBreaker {
    inner: Arc<CircuitBreakerInner>,
}

struct CircuitBreakerInner {
    state: AtomicU32,
    consecutive_failures: AtomicU32,
    last_failure_epoch: AtomicU64,
    failure_threshold: u32,
    cooldown_secs: u64,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreaker {
    /// Create a new circuit breaker, reading thresholds from env vars.
    ///
    /// - `CB_FAILURE_THRESHOLD` (default 5): consecutive failures before tripping
    /// - `CB_COOLDOWN_SECS` (default 30): seconds before allowing a probe
    pub fn new() -> Self {
        let failure_threshold = std::env::var("CB_FAILURE_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let cooldown_secs = std::env::var("CB_COOLDOWN_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        Self {
            inner: Arc::new(CircuitBreakerInner {
                state: AtomicU32::new(CLOSED),
                consecutive_failures: AtomicU32::new(0),
                last_failure_epoch: AtomicU64::new(0),
                failure_threshold,
                cooldown_secs,
            }),
        }
    }

    /// Check if a request is allowed through. Returns `Err(CircuitOpen)` if the
    /// breaker is open and the cooldown has not elapsed.
    ///
    /// When the cooldown expires, transitions to HALF_OPEN and allows one probe.
    pub fn check(&self) -> Result<(), AppError> {
        let state = self.inner.state.load(Ordering::SeqCst);

        match state {
            CLOSED => Ok(()),
            OPEN => {
                let now = now_epoch_secs();
                let last_failure = self.inner.last_failure_epoch.load(Ordering::SeqCst);

                if now.saturating_sub(last_failure) >= self.inner.cooldown_secs {
                    // Cooldown elapsed — attempt transition to half-open.
                    // Only one thread wins the CAS; others still get rejected.
                    if self
                        .inner
                        .state
                        .compare_exchange(OPEN, HALF_OPEN, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        tracing::info!("Circuit breaker → HALF_OPEN (allowing probe)");
                        Ok(())
                    } else {
                        Err(AppError::CircuitOpen)
                    }
                } else {
                    Err(AppError::CircuitOpen)
                }
            }
            HALF_OPEN => {
                // Only one probe allowed — reject additional requests while probing.
                Err(AppError::CircuitOpen)
            }
            _ => Ok(()),
        }
    }

    /// Record a successful operation. Resets failures and closes the breaker.
    pub fn record_success(&self) {
        let prev = self.inner.state.swap(CLOSED, Ordering::SeqCst);
        self.inner.consecutive_failures.store(0, Ordering::SeqCst);

        if prev != CLOSED {
            tracing::info!("Circuit breaker → CLOSED (success)");
        }
    }

    /// Record a failed operation. Increments consecutive failures and may trip.
    pub fn record_failure(&self) {
        let failures = self
            .inner
            .consecutive_failures
            .fetch_add(1, Ordering::SeqCst)
            + 1;
        self.inner
            .last_failure_epoch
            .store(now_epoch_secs(), Ordering::SeqCst);

        if failures >= self.inner.failure_threshold {
            let prev = self.inner.state.swap(OPEN, Ordering::SeqCst);
            if prev != OPEN {
                tracing::error!(
                    failures,
                    threshold = self.inner.failure_threshold,
                    "Circuit breaker → OPEN"
                );
                metrics::counter!("db_circuit_breaker_trips_total").increment(1);
            }
        }
    }

    /// Numeric gauge value for Prometheus: 0 = closed, 1 = open, 2 = half-open.
    pub fn state_gauge(&self) -> f64 {
        self.inner.state.load(Ordering::Relaxed) as f64
    }
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cb(threshold: u32, cooldown_secs: u64) -> CircuitBreaker {
        CircuitBreaker {
            inner: Arc::new(CircuitBreakerInner {
                state: AtomicU32::new(CLOSED),
                consecutive_failures: AtomicU32::new(0),
                last_failure_epoch: AtomicU64::new(0),
                failure_threshold: threshold,
                cooldown_secs,
            }),
        }
    }

    #[test]
    fn starts_closed() {
        let cb = make_cb(3, 30);
        assert!(cb.check().is_ok());
        assert_eq!(cb.state_gauge(), 0.0);
    }

    #[test]
    fn trips_after_threshold() {
        let cb = make_cb(3, 30);
        cb.record_failure();
        cb.record_failure();
        assert!(cb.check().is_ok()); // 2 failures, threshold is 3
        cb.record_failure();
        assert!(cb.check().is_err()); // 3 failures → OPEN
        assert_eq!(cb.state_gauge(), 1.0);
    }

    #[test]
    fn success_resets_failures() {
        let cb = make_cb(3, 30);
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        // Failures reset, so 3 more needed
        cb.record_failure();
        cb.record_failure();
        assert!(cb.check().is_ok());
    }

    #[test]
    fn half_open_after_cooldown() {
        let cb = make_cb(2, 0); // 0-second cooldown for test
        cb.record_failure();
        cb.record_failure(); // → OPEN

        // With 0-second cooldown, the first check after tripping immediately
        // sees the cooldown has elapsed and transitions OPEN → HALF_OPEN,
        // allowing exactly one probe request through.
        assert!(cb.check().is_ok()); // → HALF_OPEN (probe allowed)
        assert_eq!(cb.state_gauge(), 2.0);
        assert!(cb.check().is_err()); // second request during HALF_OPEN is rejected
    }

    #[test]
    fn successful_probe_closes_circuit() {
        let cb = make_cb(2, 0);
        cb.record_failure();
        cb.record_failure(); // → OPEN
        let _ = cb.check(); // still OPEN, but cooldown is 0 so...
        let _ = cb.check(); // → HALF_OPEN
        cb.record_success(); // → CLOSED
        assert_eq!(cb.state_gauge(), 0.0);
        assert!(cb.check().is_ok());
    }

    #[test]
    fn failed_probe_reopens_circuit() {
        let cb = make_cb(2, 0);
        cb.record_failure();
        cb.record_failure(); // → OPEN
        let _ = cb.check(); // transition to HALF_OPEN on next
        let _ = cb.check(); // → HALF_OPEN, allow probe
        cb.record_failure(); // probe fails → back to OPEN
        // Note: total failures is now 3, which is >= threshold 2, so it stays OPEN
        assert_eq!(cb.state_gauge(), 1.0);
    }
}
