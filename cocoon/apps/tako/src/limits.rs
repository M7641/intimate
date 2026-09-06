//! Operational limits for the ingest service, kept in one place so the peak
//! memory ceiling and the request lifetime can be reasoned about at a glance.
//!
//! # Peak memory
//!
//! At peak a single `/upload` holds, roughly at the same time:
//!   - the source bytes (up to [`MAX_FILE_SIZE`]),
//!   - the decoded Polars `DataFrame`,
//!   - the encoded Parquet buffer.
//!
//! We model that as [`MEMORY_PER_UPLOAD_FACTOR`] × [`MAX_FILE_SIZE`] per upload,
//! and at most [`max_concurrent_uploads`] uploads run at once (the upload
//! semaphore enforces this), so the application's upload memory ceiling is:
//!
//! ```text
//! peak ≈ MEMORY_PER_UPLOAD_FACTOR × MAX_FILE_SIZE × max_concurrent_uploads
//! ```
//!
//! [`log_startup_summary`] prints the resolved numbers at boot.

use std::time::Duration;
use tracing::info;

/// Largest accepted file (uncompressed source bytes) — 100 MB. Enforced by the
/// precise post-buffer check in the upload handler.
pub const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Hard cap on the whole multipart request body, enforced by `DefaultBodyLimit`
/// on the `/upload` route so an oversized body is rejected *while streaming*
/// rather than after being buffered into memory. It sits a little above
/// [`MAX_FILE_SIZE`] to leave room for the multipart envelope (boundaries and
/// per-field headers), so a legitimately-sized file still reaches the precise
/// [`MAX_FILE_SIZE`] check, which returns a clean 413 keyed to the file name.
pub const MAX_UPLOAD_BODY_SIZE: usize = MAX_FILE_SIZE + 1024 * 1024;

/// Rough multiple of the source size that one in-flight upload holds at peak
/// (source bytes + `DataFrame` + Parquet buffer coexisting). A deliberate
/// over-estimate so the computed ceiling errs on the safe side.
pub const MEMORY_PER_UPLOAD_FACTOR: usize = 3;

/// Default cap on uploads processed at once (overridable via
/// `MAX_CONCURRENT_UPLOADS`). Bounds peak memory (see module docs) and aligns
/// with the 4-connection warehouse pool, so concurrent COPYs do not outrun
/// their connections.
pub const DEFAULT_MAX_CONCURRENT_UPLOADS: usize = 4;

/// Default per-request timeout for `/upload` (overridable via
/// `UPLOAD_TIMEOUT_SECS`). Generous enough for a large file plus a warehouse
/// COPY, but bounds a stalled request from holding a concurrency permit forever.
pub const DEFAULT_UPLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// Default warehouse connection-pool size (overridable via `DB_MAX_CONNECTIONS`,
/// the same name `service-kit` uses). Each in-flight upload holds one connection
/// for its COPY, so a pool smaller than [`DEFAULT_MAX_CONCURRENT_UPLOADS`] makes
/// the pool — not the upload semaphore — the bottleneck. They default to the
/// same value so they match out of the box.
pub const DEFAULT_DB_POOL_SIZE: usize = 4;

/// Default server-side warehouse statement timeout (overridable via
/// `DB_STATEMENT_TIMEOUT_SECS`; set it to `0` to disable). This is what actually
/// aborts a runaway COPY warehouse-side and frees the pooled connection — the
/// request-level [`upload_timeout`] only drops the handler future, not the
/// detached blocking COPY. Matched to the upload timeout so the two agree.
pub const DEFAULT_DB_STATEMENT_TIMEOUT: Duration = Duration::from_secs(120);

/// Default global request-rate cap, in requests per second, applied to the
/// load-bearing routes (`/upload`, `/table*`). This is a coarse *defensive*
/// backstop on total throughput — per-client fairness stays the gateway's job.
/// Overridable via `RATE_LIMIT_RPS`.
pub const DEFAULT_RATE_LIMIT_RPS: usize = 100;

/// Read a positive `usize` from `name`, falling back to `default` when the var
/// is unset, unparseable, or zero.
fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(default)
}

/// Resolved upload concurrency cap (env `MAX_CONCURRENT_UPLOADS`, else default).
pub fn max_concurrent_uploads() -> usize {
    env_usize("MAX_CONCURRENT_UPLOADS", DEFAULT_MAX_CONCURRENT_UPLOADS)
}

/// Resolved per-request upload timeout (env `UPLOAD_TIMEOUT_SECS`, else default).
pub fn upload_timeout() -> Duration {
    std::env::var("UPLOAD_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|&n| n > 0)
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_UPLOAD_TIMEOUT)
}

/// Resolved warehouse connection-pool size (env `DB_MAX_CONNECTIONS`, else default).
pub fn db_pool_size() -> usize {
    env_usize("DB_MAX_CONNECTIONS", DEFAULT_DB_POOL_SIZE)
}

/// Resolved global request-rate cap, req/s (env `RATE_LIMIT_RPS`, else default).
pub fn rate_limit_rps() -> usize {
    env_usize("RATE_LIMIT_RPS", DEFAULT_RATE_LIMIT_RPS)
}

/// Resolved warehouse statement timeout. Unset → [`DEFAULT_DB_STATEMENT_TIMEOUT`];
/// `DB_STATEMENT_TIMEOUT_SECS=0` → `None` (disabled); any positive value → that
/// many seconds.
pub fn db_statement_timeout() -> Option<Duration> {
    match std::env::var("DB_STATEMENT_TIMEOUT_SECS")
        .ok()
        .map(|v| v.parse::<u64>())
    {
        Some(Ok(0)) => None,
        Some(Ok(secs)) => Some(Duration::from_secs(secs)),
        _ => Some(DEFAULT_DB_STATEMENT_TIMEOUT),
    }
}

/// Upper bound on memory held by all in-flight uploads at the given concurrency.
pub fn estimated_peak_memory_bytes(max_concurrent_uploads: usize) -> usize {
    MEMORY_PER_UPLOAD_FACTOR * MAX_FILE_SIZE * max_concurrent_uploads
}

/// Log the resolved limits once at startup, including the derived memory ceiling.
pub fn log_startup_summary() {
    let concurrency = max_concurrent_uploads();
    info!(
        max_file_mb = MAX_FILE_SIZE / 1024 / 1024,
        max_concurrent_uploads = concurrency,
        rate_limit_rps = rate_limit_rps(),
        db_pool_size = db_pool_size(),
        upload_timeout_secs = upload_timeout().as_secs(),
        // 0 == disabled
        db_statement_timeout_secs = db_statement_timeout().map(|d| d.as_secs()).unwrap_or(0),
        estimated_peak_memory_mb = estimated_peak_memory_bytes(concurrency) / 1024 / 1024,
        "Resource limits"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_cap_exceeds_file_size_for_the_multipart_envelope() {
        // The streaming body cap must leave headroom above the precise file-size
        // check, or a legitimately-sized file would be rejected by the layer.
        assert!(MAX_UPLOAD_BODY_SIZE > MAX_FILE_SIZE);
    }

    #[test]
    fn peak_memory_scales_with_concurrency() {
        assert_eq!(
            estimated_peak_memory_bytes(1),
            MEMORY_PER_UPLOAD_FACTOR * MAX_FILE_SIZE
        );
        assert_eq!(
            estimated_peak_memory_bytes(4),
            4 * estimated_peak_memory_bytes(1)
        );
        assert_eq!(estimated_peak_memory_bytes(0), 0);
    }
}
