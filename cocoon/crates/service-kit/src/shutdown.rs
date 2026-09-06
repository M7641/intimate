//! Graceful-shutdown signal shared by the cocoon axum services.

/// Resolves once the process receives a shutdown signal.
///
/// Listens for Ctrl+C on every platform, and additionally for `SIGTERM` on
/// unix — the signal Kubernetes and Docker send to terminate a container.
/// Pass it to [`axum::serve`]'s `with_graceful_shutdown` so in-flight requests
/// drain before the listener stops accepting new ones.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(unix)]
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    #[cfg(not(unix))]
    ctrl_c.await;

    tracing::info!("Shutdown signal received, starting graceful shutdown");
}
