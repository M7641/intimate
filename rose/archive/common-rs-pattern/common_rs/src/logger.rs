use tracing_subscriber::{EnvFilter, fmt};

/// Initialise tracing with an env-filter. Defaults to `info` if `RUST_LOG` is unset.
///
/// Call once near the start of `main()`. Idempotent-safe to call multiple times only via
/// `try_init` — production callers should call it exactly once.
pub fn init_tracing(default_directive: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_directive));

    fmt().with_env_filter(filter).init();
}
