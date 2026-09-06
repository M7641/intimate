# Manual Shutdown

I think the context for this was when profiling the entire application I did not have a way to turn the profiling off and so as a way to kill the server without having to kill all processes on a port we had this.

```rust

// Global shutdown notifier for profiling
static SHUTDOWN_NOTIFIER: OnceLock<Arc<tokio::sync::Notify>> = OnceLock::new();

pub fn get_shutdown_notifier() -> Option<Arc<tokio::sync::Notify>> {
    SHUTDOWN_NOTIFIER.get().cloned()
}
// Add shutdown endpoint for profiling (only when ENABLE_SHUTDOWN_ENDPOINT=true)
let shut_down_enabled = match std::env::var("ENABLE_SHUTDOWN_ENDPOINT") {
    Ok(_) => true,
    Err(_) => false,
};

if shut_down_enabled {
    use tokio::sync::Notify;

    let shutdown_notify = Arc::new(Notify::new());
    let shutdown_notify_clone = shutdown_notify.clone();

    router = router.route(
        "/shutdown",
        get(move || {
            let notify = shutdown_notify_clone.clone();
            async move {
                tracing::warn!("Shutdown requested via /shutdown endpoint");
                // Trigger shutdown after a brief delay to allow response to be sent
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    notify.notify_one();
                });
                (StatusCode::OK, "Shutting down gracefully...")
            }
        }),
    );

    // Store the shutdown notifier so main.rs can use it
    SHUTDOWN_NOTIFIER.set(shutdown_notify).ok();
}
```
