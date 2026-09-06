//! A small Rama app — another Rust web server, to compare footprints.
//!
//! Rama is a modular service framework (the same one used for proxies); its HTTP
//! server is built from composable tower-style services. Same endpoint shapes as
//! the Axum and FastAPI examples so gauge measures it identically. The point is
//! the comparison: how does Rama's resting footprint sit next to Axum's and
//! Python's?

use rama::http::server::HttpServer;
use rama::http::service::web::extract::Path;
use rama::http::service::web::response::Json;
use rama::http::service::web::WebService;
use rama::rt::Executor;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Mutex;

// Grows on every /work request → a visible memory climb, like the other examples.
static CACHE: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());

#[derive(Deserialize)]
struct ItemParams {
    id: u64,
}

async fn work() -> Json<Value> {
    // CPU: a tight arithmetic burn, returned so it can't be optimized away.
    let mut acc: u64 = 0;
    for _ in 0..2_000_000u64 {
        acc = acc
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
    }
    // Memory: retain ~256 KB, touching every page so it is actually resident
    // (see the Axum example's note on lazy zero pages).
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
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);
    let host = std::env::var("GAUGE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

    let service = WebService::default()
        .with_get("/health", async || Json(json!({ "status": "ok" })))
        .with_get("/items/{id}", async |Path(p): Path<ItemParams>| {
            Json(json!({ "item_id": p.id, "name": format!("item-{}", p.id) }))
        })
        .with_get("/work", async || work().await);

    HttpServer::auto(Executor::default())
        .listen(format!("{host}:{port}"), service)
        .await
        .unwrap();
}
