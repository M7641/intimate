//! A small Axum app — the Rust counterpart to the FastAPI example.
//!
//! Same endpoint shapes (/health, /items/{id}, /search, /work) so it can be
//! measured the same way. The point of having it is contrast: a Rust service's
//! baseline RAM is a fraction of a Python framework's, which gauge makes visible.
//!
//! Build with `--features dhat-heap` to profile the heap (see Dockerfile.dhat):
//! a global allocator records every allocation, and dhat writes `dhat-heap.json`
//! on graceful shutdown — the Rust analogue of memray.

// The dhat global allocator is installed only when profiling, so the normal
// build pays nothing for it.
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use axum::{
    extract::{Path, Query},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Mutex;

// Grows on every /work request → a visible memory climb, like the Python cache.
static CACHE: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn get_item(Path(id): Path<u64>) -> Json<Value> {
    Json(json!({ "item_id": id, "name": format!("item-{id}") }))
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
}

async fn search(Query(p): Query<SearchParams>) -> Json<Value> {
    let results: Vec<String> = (0..5).map(|i| format!("{}-{}", p.q, i)).collect();
    Json(json!({ "query": p.q, "results": results }))
}

async fn work() -> Json<Value> {
    // CPU: a tight arithmetic burn (an LCG), returned so it can't be optimized out.
    let mut acc: u64 = 0;
    for _ in 0..2_000_000u64 {
        acc = acc
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
    }
    // Memory: retain ~256 KB, mirroring the FastAPI example's cache. We touch one
    // byte per 4 KB page: `vec![0u8; N]` is lazily backed by copy-on-write zero
    // pages, so without writing to it the memory is allocated but never resident
    // (gauge would show no RSS growth). Touching commits the pages for real.
    let mut chunk = vec![0u8; 256_000];
    for i in (0..chunk.len()).step_by(4096) {
        chunk[i] = 1;
    }
    let mut cache = CACHE.lock().unwrap();
    cache.push(chunk);
    Json(json!({ "cache": cache.len(), "acc": acc }))
}

#[tokio::main]
async fn main() {
    // On drop (at graceful shutdown) this writes dhat-heap.json to the cwd.
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);
    let host = std::env::var("GAUGE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

    let app = Router::new()
        .route("/health", get(health))
        .route("/items/{id}", get(get_item))
        .route("/search", get(search))
        .route("/work", get(work));

    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}"))
        .await
        .unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

/// Return on SIGTERM (`podman stop`) or Ctrl-C, so `main` unwinds cleanly and the
/// dhat profiler flushes its capture.
async fn shutdown_signal() {
    use tokio::signal;
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    let interrupt = async {
        signal::ctrl_c().await.expect("install Ctrl-C handler");
    };
    tokio::select! {
        _ = terminate => {}
        _ = interrupt => {}
    }
}
