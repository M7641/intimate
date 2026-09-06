//! Grower-tooling assignments + countsize coverage.
//! Port of `modules/planner/levers/backend/routes/required_tooling/route.py`.

use std::path::PathBuf;
use std::time::Duration;

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tokio::process::Command;
use tokio::time::timeout;

use common_rs::auth::AuthenticatedUser;
use common_rs::db::{BindValue, Params};

use crate::state::AppState;
use common_rs::error::{ApiError, ApiResult};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/required_tooling/",
            get(get_required_tooling).patch(save_tooling_assignments),
        )
        .route(
            "/required_tooling/options",
            get(get_required_tooling_options).post(add_tooling_option),
        )
        .route("/required_tooling/growers", get(get_growers))
        .route(
            "/required_tooling/countsize_coverage",
            get(get_countsize_coverage)
                .post(add_countsize_coverage)
                .delete(delete_countsize_coverage),
        )
        .route(
            "/required_tooling/countsize_coverage/rebuild",
            post(rebuild_tooling_model),
        )
}

// ---------- Grower has tooling ----------

#[derive(FromRow, Serialize)]
pub struct RequiredToolingRecord {
    pub supcode: Option<String>,
    pub required_tooling: Option<String>,
    pub active: Option<bool>,
    pub uploaded_at: Option<String>,
}

#[derive(Serialize)]
pub struct RequiredToolingOptionsResponse {
    pub required_tooling_options: Vec<String>,
}

#[derive(Deserialize)]
pub struct AddToolingRequest {
    pub required_tooling: String,
}

#[derive(Deserialize)]
pub struct GrowerToolingAssignment {
    pub supcode: String,
    pub required_tooling: String,
    pub active: bool,
}

#[derive(Deserialize)]
pub struct SaveToolingAssignmentsRequest {
    pub assignments: Vec<GrowerToolingAssignment>,
}

#[derive(FromRow, Serialize)]
pub struct GrowerRecord {
    pub supcode: String,
}

async fn get_required_tooling(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> ApiResult<Json<Vec<RequiredToolingRecord>>> {
    let rows: Vec<RequiredToolingRecord> = state
        .db
        .load_data(
            r#"
            SELECT
                supcode,
                required_tooling,
                active,
                uploaded_at
            FROM {schema}.lever_grower_has_required_tooling
        "#,
            &Params::new(),
        )
        .await?;
    Ok(Json(rows))
}

#[derive(FromRow)]
struct OptionRow {
    required_tooling: Option<String>,
}

async fn get_required_tooling_options(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> ApiResult<Json<RequiredToolingOptionsResponse>> {
    let rows: Vec<OptionRow> = state
        .db
        .load_data(
            r#"
            SELECT DISTINCT required_tooling
            FROM {schema}.lever_possible_required_tooling
            WHERE required_tooling IS NOT NULL
        "#,
            &Params::new(),
        )
        .await?;
    Ok(Json(RequiredToolingOptionsResponse {
        required_tooling_options: rows
            .into_iter()
            .filter_map(|r| r.required_tooling)
            .collect(),
    }))
}

async fn add_tooling_option(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(body): Json<AddToolingRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let tooling_name = body.required_tooling.trim().to_string();

    if tooling_name.is_empty() {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "message": "Tooling name cannot be empty"
        })));
    }

    let mut params = Params::new();
    params.insert(
        "tooling_name".to_string(),
        BindValue::Text(tooling_name.clone()),
    );

    let existing: Vec<OptionRow> = state
        .db
        .load_data(
            r#"
            SELECT required_tooling
            FROM {schema}.lever_possible_required_tooling
            WHERE upper(trim(required_tooling)) = upper(trim(%(tooling_name)s))
        "#,
            &params,
        )
        .await?;

    if !existing.is_empty() {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "message": format!("Tooling option '{tooling_name}' already exists")
        })));
    }

    state
        .db
        .execute_query(
            r#"
            INSERT INTO {schema}.lever_possible_required_tooling (required_tooling)
            VALUES (%(tooling_name)s)
        "#,
            &params,
        )
        .await?;

    tracing::info!("Added new tooling option: {tooling_name}");
    Ok(Json(serde_json::json!({
        "status": "success",
        "message": format!("Added tooling option '{tooling_name}'")
    })))
}

async fn get_growers(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> ApiResult<Json<Vec<GrowerRecord>>> {
    let rows: Vec<GrowerRecord> = state
        .db
        .load_data(
            r#"
            SELECT DISTINCT upper(trim(supcode)) AS supcode
            FROM {schema}.daily_suppliers
            WHERE supcode IS NOT NULL
                AND source = 'domestic'
            ORDER BY supcode
        "#,
            &Params::new(),
        )
        .await?;
    Ok(Json(rows))
}

async fn save_tooling_assignments(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(body): Json<SaveToolingAssignmentsRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    if body.assignments.is_empty() {
        return Ok(Json(serde_json::json!({
            "status": "success",
            "message": "No assignments to save"
        })));
    }

    let schema = state.db.schema();
    let uploaded_at = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let n = body.assignments.len();

    state
        .db
        .execute_query(
            r#"
            CREATE TABLE IF NOT EXISTS {schema}.lever_grower_has_required_tooling_staging (
                supcode VARCHAR(64) NOT NULL,
                required_tooling VARCHAR(256) NOT NULL,
                active BOOLEAN DEFAULT TRUE,
                uploaded_at VARCHAR(64)
            );
        "#,
            &Params::new(),
        )
        .await?;

    // Bulk-insert into the staging table. Mirrors the python `db.insert_data`
    // that goes through a polars DataFrame — we just build a single multi-row
    // INSERT with positional placeholders.
    let cols_per_row: usize = 4;
    let placeholder_chunks: Vec<String> = (0..n)
        .map(|i| {
            let base = i * cols_per_row;
            let phs: Vec<String> = (1..=cols_per_row)
                .map(|j| format!("${}", base + j))
                .collect();
            format!("({})", phs.join(", "))
        })
        .collect();

    let insert_sql = format!(
        "INSERT INTO {schema}.lever_grower_has_required_tooling_staging
            (supcode, required_tooling, active, uploaded_at)
         VALUES {}",
        placeholder_chunks.join(", ")
    );

    let mut q = sqlx::query(&insert_sql);
    for a in &body.assignments {
        q = q.bind(a.supcode.clone());
        q = q.bind(a.required_tooling.clone());
        q = q.bind(a.active);
        q = q.bind(uploaded_at.clone());
    }
    q.execute(state.db.pool()).await?;

    state.db.execute_query(
        r#"
            MERGE INTO {schema}.lever_grower_has_required_tooling
            USING {schema}.lever_grower_has_required_tooling_staging AS source
                ON {schema}.lever_grower_has_required_tooling.supcode = source.supcode
                AND {schema}.lever_grower_has_required_tooling.required_tooling = source.required_tooling
            WHEN MATCHED THEN
                UPDATE SET
                    active = source.active,
                    uploaded_at = source.uploaded_at
            WHEN NOT MATCHED THEN
                INSERT (supcode, required_tooling, active, uploaded_at)
                VALUES (source.supcode, source.required_tooling, source.active, source.uploaded_at);
        "#,
        &Params::new(),
    )
    .await?;

    state
        .db
        .execute_query(
            "TRUNCATE TABLE {schema}.lever_grower_has_required_tooling_staging;",
            &Params::new(),
        )
        .await?;

    tracing::info!("Saved {n} tooling assignment records with timestamp {uploaded_at}");

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": format!("Saved {n} tooling assignments")
    })))
}

// ---------- Countsize coverage ----------

#[derive(FromRow, Serialize)]
pub struct ToolingCountsizeCoverage {
    pub tool_required: Option<String>,
    pub countsize: Option<String>,
    pub source: Option<String>,
}

#[derive(Deserialize)]
pub struct AddToolingCountsizeRequest {
    pub tool_required: String,
    pub countsize: String,
}

#[derive(Deserialize)]
pub struct DeleteToolingCountsizeRequest {
    pub tool_required: String,
    pub countsize: String,
}

async fn get_countsize_coverage(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> ApiResult<Json<Vec<ToolingCountsizeCoverage>>> {
    let rows: Vec<ToolingCountsizeCoverage> = state
        .db
        .load_data(
            r#"
            SELECT DISTINCT
                tool_required,
                countsize,
                source
            FROM {schema}.weekly_required_tooling
            WHERE tool_required IS NOT NULL
              AND countsize IS NOT NULL
            ORDER BY tool_required, countsize
        "#,
            &Params::new(),
        )
        .await?;
    Ok(Json(rows))
}

#[derive(FromRow)]
struct OneRow {
    #[allow(dead_code)]
    one: i32,
}

async fn add_countsize_coverage(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(body): Json<AddToolingCountsizeRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let tool = body.tool_required.trim().to_string();
    let countsize = body.countsize.trim().to_string();

    if tool.is_empty() || countsize.is_empty() {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "message": "tool_required and countsize cannot be empty"
        })));
    }

    let mut params = Params::new();
    params.insert("tool".to_string(), BindValue::Text(tool.clone()));
    params.insert("countsize".to_string(), BindValue::Text(countsize.clone()));

    let existing: Vec<OneRow> = state
        .db
        .load_data(
            r#"
            SELECT 1 AS one
            FROM {schema}.lever_tooling_countsize_coverage
            WHERE upper(trim(tool_required)) = upper(trim(%(tool)s))
              AND upper(trim(countsize)) = upper(trim(%(countsize)s))
        "#,
            &params,
        )
        .await?;

    if !existing.is_empty() {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "message": format!("Coverage '{tool} -> {countsize}' already exists")
        })));
    }

    let created_at = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    params.insert("created_at".to_string(), BindValue::Text(created_at));

    state
        .db
        .execute_query(
            r#"
            INSERT INTO {schema}.lever_tooling_countsize_coverage
                (tool_required, countsize, created_at)
            VALUES (%(tool)s, %(countsize)s, %(created_at)s)
        "#,
            &params,
        )
        .await?;

    tracing::info!("Added countsize coverage: {tool} -> {countsize}");
    Ok(Json(serde_json::json!({
        "status": "success",
        "message": format!("Added coverage '{tool} -> {countsize}'")
    })))
}

async fn delete_countsize_coverage(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(body): Json<DeleteToolingCountsizeRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let tool = body.tool_required.trim().to_string();
    let countsize = body.countsize.trim().to_string();

    let mut params = Params::new();
    params.insert("tool".to_string(), BindValue::Text(tool.clone()));
    params.insert("countsize".to_string(), BindValue::Text(countsize.clone()));

    state
        .db
        .execute_query(
            r#"
            DELETE FROM {schema}.lever_tooling_countsize_coverage
            WHERE upper(trim(tool_required)) = upper(trim(%(tool)s))
              AND upper(trim(countsize)) = upper(trim(%(countsize)s))
        "#,
            &params,
        )
        .await?;

    tracing::info!("Deleted countsize coverage: {tool} -> {countsize}");
    Ok(Json(serde_json::json!({
        "status": "success",
        "message": format!("Deleted coverage '{tool} -> {countsize}'")
    })))
}

/// Resolves the repo-level `dbt/` folder. Override with `LEVERS_DBT_PATH` for
/// deployments that mount the dbt project elsewhere.
fn dbt_path() -> PathBuf {
    if let Ok(p) = std::env::var("LEVERS_DBT_PATH") {
        return PathBuf::from(p);
    }
    // backend → levers (inner) → levers (outer) → modules → repo root
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..4 {
        p = p
            .parent()
            .expect("manifest dir has 4 ancestors up to repo root")
            .to_path_buf();
    }
    p.join("dbt")
}

async fn rebuild_tooling_model(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> ApiResult<Json<serde_json::Value>> {
    let dbt_folder = dbt_path();
    let target = state.env.target().as_str();

    let fut = Command::new("dbt")
        .args([
            "run",
            "-t",
            target,
            "-s",
            "weekly_required_tooling",
            "--profiles-dir",
            dbt_folder.to_str().unwrap_or("dbt"),
            "--project-dir",
            dbt_folder.to_str().unwrap_or("dbt"),
        ])
        .output();

    let output = match timeout(Duration::from_secs(120), fut).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            tracing::error!("dbt rebuild failed to spawn: {e}");
            return Err(ApiError::Internal(format!("dbt rebuild failed: {e}")));
        }
        Err(_) => {
            tracing::error!("dbt rebuild timed out after 120s");
            return Err(ApiError::Internal("dbt rebuild timed out".to_string()));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail = stderr
            .chars()
            .rev()
            .take(500)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        tracing::error!("dbt rebuild failed: {tail}");
        return Err(ApiError::Internal(format!("dbt rebuild failed: {tail}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let tail = stdout
        .chars()
        .rev()
        .take(500)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    tracing::info!("dbt rebuild succeeded: {tail}");

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Tooling model rebuilt successfully"
    })))
}
