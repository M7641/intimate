# Graceful Shutdown

> Status: **Implemented** — `src/main.rs`

## What

When the process receives `SIGTERM` (Kubernetes pod termination) or `Ctrl+C` (interactive stop):

1. **Stop accepting** new connections
2. **Drain** in-flight requests — let them complete naturally
3. **Flush** buffered telemetry (logs, traces, metrics)
4. **Exit** cleanly with status 0

## Why

Without graceful shutdown, deployment is destructive:

| Event                         | Without graceful shutdown                                                       | With graceful shutdown                               |
| ----------------------------- | ------------------------------------------------------------------------------- | ---------------------------------------------------- |
| `kubectl rollout restart`     | Active requests receive TCP RST → users see 502                                 | Active requests complete normally → zero user impact |
| Prometheus scrape in progress | Scrape fails, gap in metrics                                                    | Scrape completes, metrics are continuous             |
| Log buffer has 500 lines      | Lines lost — you're blind to what happened right before shutdown                | Buffer flushed — complete audit trail                |
| DB transactions in progress   | Connections severed → possible data corruption, stale pool slots on the DB side | Transactions complete, connections returned to pool  |

In Kubernetes, a rolling deployment sends `SIGTERM`, waits `terminationGracePeriodSeconds` (default 30s), then sends `SIGKILL`. If your service drains within that window, deployments are invisible to users.

## How — This Repo

```rust
axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("Server error");

async fn shutdown_signal() {
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

    tracing::info!("Shutdown signal received, draining connections...");
}
```

### Key design decisions

- **Both SIGTERM and Ctrl+C**: SIGTERM is what Kubernetes sends; Ctrl+C is what developers use locally. Handle both.
- **`#[cfg(unix)]`**: SIGTERM handling compiles only on Unix (macOS, Linux). Windows falls back to Ctrl+C only.
- **Log on shutdown**: the "draining connections" message confirms the signal was received. If this line is missing from logs, the process was killed before the handler ran.
