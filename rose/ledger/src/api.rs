//! HTTP surface: one endpoint to explain a query across every engine, one to
//! list which engines are available.

use crate::engine::Registry;
use crate::plan::PlanResult;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct AppState {
    pub registry: Registry,
}

pub fn router(state: AppState) -> Router {
    let mut app = Router::new()
        .route("/api/engines", get(list_engines))
        .route("/api/explain", post(explain));

    // Serve the built frontend when it exists, so `ledger serve` is a single
    // process in production; in dev the Vite server proxies /api here instead.
    if std::path::Path::new("frontend/dist").is_dir() {
        app = app.fallback_service(ServeDir::new("frontend/dist"));
    }

    app.layer(CorsLayer::permissive()).with_state(state)
}

#[derive(Serialize)]
struct EngineInfo {
    id: String,
    name: String,
    kind: String,
}

async fn list_engines(State(state): State<AppState>) -> Json<Vec<EngineInfo>> {
    let engines = state
        .registry
        .iter()
        .map(|e| EngineInfo {
            id: e.id().to_string(),
            name: e.name().to_string(),
            kind: e.kind().label().to_string(),
        })
        .collect();
    Json(engines)
}

#[derive(Deserialize)]
struct ExplainRequest {
    sql: String,
    /// When true, engines actually execute the query to gather real timing.
    #[serde(default)]
    analyze: bool,
}

/// One engine's answer. `ok = false` carries the error message (e.g. a
/// dialect Postgres accepts but DuckDB rejects) without failing the others.
#[derive(Serialize)]
struct EngineOutcome {
    engine: String,
    name: String,
    kind: String,
    ok: bool,
    error: Option<String>,
    plan: Option<PlanResult>,
}

#[derive(Serialize)]
struct ExplainResponse {
    sql: String,
    analyzed: bool,
    engines: Vec<EngineOutcome>,
}

async fn explain(
    State(state): State<AppState>,
    Json(req): Json<ExplainRequest>,
) -> Json<ExplainResponse> {
    let analyze = req.analyze;
    let sql = req.sql;

    // Explain on every engine concurrently — they are independent backends.
    let futures = state.registry.iter().map(|engine| {
        let sql = sql.clone();
        async move {
            let result = engine.explain(&sql, analyze).await;
            match result {
                Ok(plan) => EngineOutcome {
                    engine: engine.id().to_string(),
                    name: engine.name().to_string(),
                    kind: engine.kind().label().to_string(),
                    ok: true,
                    error: None,
                    plan: Some(plan),
                },
                Err(e) => EngineOutcome {
                    engine: engine.id().to_string(),
                    name: engine.name().to_string(),
                    kind: engine.kind().label().to_string(),
                    ok: false,
                    // The full chain (e.g. dialect detail from Postgres) is the
                    // useful part to show the user.
                    error: Some(format!("{e:#}")),
                    plan: None,
                },
            }
        }
    });

    let engines = futures::future::join_all(futures).await;
    Json(ExplainResponse { sql, analyzed: analyze, engines })
}
