//! Prometheus metrics setup, decoupled from any particular `AppState`.
//!
//! [`install_recorder`] wires the process-global recorder that
//! [`crate::middleware::metrics_layer`] records into, and returns the handle that
//! renders the exposition format for a `/metrics` endpoint. Apps hold that handle
//! wherever they keep shared state — they do not need `service_kit::AppState` to
//! get metrics.

use metrics_exporter_prometheus::PrometheusBuilder;
pub use metrics_exporter_prometheus::PrometheusHandle;

/// Install the process-global Prometheus recorder and return a handle for
/// rendering it.
///
/// **Idempotent**: there can be only one global recorder, so a second call is a
/// no-op for the global state (only the first recorder is live) but still returns
/// a valid — though inert — handle. This keeps it safe to call from tests that
/// build the application state more than once in a single process.
pub fn install_recorder() -> PrometheusHandle {
    let recorder = PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    // Ignore "a recorder is already installed": the first install wins, later
    // calls just get an inert handle. Construction itself never fails.
    let _ = metrics::set_global_recorder(recorder);
    handle
}
