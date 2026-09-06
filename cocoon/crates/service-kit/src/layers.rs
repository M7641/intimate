//! Standard tower-http layers shared by the cocoon axum services.
//!
//! These are not specific to any app — they are the common observability,
//! robustness and transport layers a service wants. Apps add them to their
//! router, e.g. `.layer(service_kit::layers::catch_panic_layer())`. Keeping the
//! constructors here means the (verbose, generic) tower-http types and feature
//! flags live in one place instead of being re-derived per app.

use tower_http::catch_panic::{CatchPanicLayer, DefaultResponseForPanic};
use tower_http::classify::{ServerErrorsAsFailures, SharedClassifier};
use tower_http::compression::CompressionLayer;
use tower_http::compression::predicate::SizeAbove;
use tower_http::trace::TraceLayer;

/// Per-request tracing: a span (method, path) plus a response event carrying the
/// status and latency. Nest it inside the `request_id` layer so the request id
/// propagates into the trace's events.
pub fn trace_layer() -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>> {
    TraceLayer::new_for_http()
}

/// Turn a panicking handler into a generic `500` (logged) instead of letting the
/// panic abort the connection's task. A cheap, per-request safety net — useful
/// wherever a handler touches third-party code that might panic on bad input.
pub fn catch_panic_layer() -> CatchPanicLayer<DefaultResponseForPanic> {
    CatchPanicLayer::new()
}

/// Default body-size threshold, in bytes, below which responses are not
/// compressed: small payloads cost more to compress than they save.
pub const DEFAULT_COMPRESSION_MIN_SIZE: u16 = 1024;

/// gzip response compression that only engages for bodies of at least
/// `min_size_bytes` (judged from `Content-Length`), so tiny JSON responses stay
/// uncompressed. Only compresses when the client advertises `Accept-Encoding`.
///
/// `min_size_bytes` is a `u16` (tower-http's `SizeAbove`), so the threshold tops
/// out at 64 KiB — fine for "skip the small stuff", which is its purpose.
pub fn compression_layer(min_size_bytes: u16) -> CompressionLayer<SizeAbove> {
    CompressionLayer::new().compress_when(SizeAbove::new(min_size_bytes))
}
