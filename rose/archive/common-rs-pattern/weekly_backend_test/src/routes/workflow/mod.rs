//! Workflow execution and run-status endpoints for the weekly optimiser.

use std::collections::HashMap;
use std::sync::LazyLock;

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use common_rs::db::{BindValue, Params};
use common_rs::env::Target;
use common_rs::nimbus_workflow::NimbusWorkflowClient;

use crate::state::AppState;
use common_rs::auth::AuthenticatedUser;
use common_rs::error::{ApiError, ApiResult};

static WORKFLOW_NAMES: LazyLock<HashMap<Target, String>> = LazyLock::new(|| {
    HashMap::from([
        (Target::Prod, "weekly-run-optimiser-v2-prod".to_string()),
        (Target::Test, "weekly-run-optimiser-v2-test".to_string()),
        (Target::Dev, "weekly-run-optimiser-v2-dev".to_string()),
    ])
});

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/workflow/has_model_ran_today", get(has_model_ran_today))
        .route(
            "/workflow/latest_workflow_run_person",
            get(latest_workflow_run_person),
        )
        .route(
            "/workflow/latest_workflow_details",
            get(latest_workflow_details),
        )
        .route(
            "/workflow/execute_workflow",
            post(execute_workflow_endpoint),
        )
}

#[derive(Serialize, FromRow)]
struct LastRunRow {
    save_timestamp: chrono::NaiveDateTime,
    status_message: Option<String>,
}

#[derive(Serialize)]
pub struct HasModelRanTodayResponse {
    pub has_model_ran_today: bool,
    pub status_message: Option<String>,
}

#[derive(Serialize)]
pub struct LatestRunPersonResponse {
    pub latest_user_to_run: String,
}

#[derive(Deserialize)]
pub struct ExecuteWorkflowRequest {
    #[serde(default)]
    pub use_latest_supply_levers: bool,
    #[serde(default)]
    pub use_latest_demand_levers: bool,
}

#[derive(Serialize)]
pub struct WorkflowExecutionResponse {
    pub message: String,
}

async fn has_model_ran_today(
    State(state): State<AppState>,
) -> ApiResult<Json<HasModelRanTodayResponse>> {
    let rows: Vec<LastRunRow> = state
        .db
        .load_data(
            r#"
            SELECT save_timestamp, status_message
            FROM {schema}.weekly_run_requests
            WHERE save_timestamp::date = CURRENT_DATE
            ORDER BY save_timestamp DESC
            LIMIT 1
            "#,
            &Params::new(),
        )
        .await?;

    Ok(Json(match rows.into_iter().next() {
        Some(r) => HasModelRanTodayResponse {
            has_model_ran_today: true,
            status_message: r.status_message,
        },
        None => HasModelRanTodayResponse {
            has_model_ran_today: false,
            status_message: None,
        },
    }))
}

#[derive(FromRow)]
struct UserRow {
    save_user: Option<String>,
}

async fn latest_workflow_run_person(
    State(state): State<AppState>,
) -> ApiResult<Json<LatestRunPersonResponse>> {
    let rows: Vec<UserRow> = state
        .db
        .load_data(
            r#"
            SELECT save_user
            FROM {schema}.weekly_run_requests
            ORDER BY save_timestamp DESC
            LIMIT 1
            "#,
            &Params::new(),
        )
        .await?;

    let user = rows
        .into_iter()
        .next()
        .and_then(|r| r.save_user)
        .unwrap_or_else(|| "No user found".to_string());
    Ok(Json(LatestRunPersonResponse {
        latest_user_to_run: user,
    }))
}

async fn latest_workflow_details(
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let client = NimbusWorkflowClient::from_env()
        .map_err(|e| ApiError::Internal(format!("peak client: {e}")))?;
    let latest = client
        .get_latest_execution(&WORKFLOW_NAMES, state.env.target())
        .await
        .map_err(|e| ApiError::Internal(format!("peak: {e}")))?
        .unwrap_or_else(|| serde_json::json!({}));
    Ok(Json(latest))
}

async fn execute_workflow_endpoint(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(body): Json<ExecuteWorkflowRequest>,
) -> ApiResult<Json<WorkflowExecutionResponse>> {
    if !user.can_run_workflow {
        return Err(ApiError::BadRequest(
            "You do not have permission to run the optimiser".to_string(),
        ));
    }

    let mut params = Params::new();
    params.insert(
        "save_timestamp".to_string(),
        BindValue::DateTime(Utc::now()),
    );
    params.insert("save_user".to_string(), BindValue::Text(user.email.clone()));
    params.insert(
        "use_latest_supply_levers".to_string(),
        BindValue::Int(if body.use_latest_supply_levers { 1 } else { 0 }),
    );
    params.insert(
        "use_latest_demand_levers".to_string(),
        BindValue::Int(if body.use_latest_demand_levers { 1 } else { 0 }),
    );

    state
        .db
        .execute_query(
            r#"
            INSERT INTO {schema}.weekly_run_requests
                (save_timestamp, save_user, use_latest_supply_levers, use_latest_demand_levers)
            VALUES (
                %(save_timestamp)s,
                %(save_user)s,
                %(use_latest_supply_levers)s,
                %(use_latest_demand_levers)s
            )
            "#,
            &params,
        )
        .await?;

    let client = NimbusWorkflowClient::from_env()
        .map_err(|e| ApiError::Internal(format!("peak client: {e}")))?;
    client
        .execute_workflow(&WORKFLOW_NAMES, state.env.target())
        .await
        .map_err(|e| ApiError::Internal(format!("peak: {e}")))?;

    Ok(Json(WorkflowExecutionResponse {
        message: "Workflow execution triggered successfully".to_string(),
    }))
}
