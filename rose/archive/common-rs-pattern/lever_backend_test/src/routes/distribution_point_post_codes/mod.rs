//! SCD2 CRUD for distribution-point × post-code pairings.
//! Port of `modules/planner/levers/backend/routes/distribution_point_post_codes/route.py`.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::sync::LazyLock;
use uuid::Uuid;

use common_rs::auth::AuthenticatedUser;
use common_rs::db::{BindValue, Params};
use common_rs::hash::generate_hash_id;
use common_rs::validation::SAFE_HEX_64;

use crate::state::AppState;
use common_rs::error::{ApiError, ApiResult};

static SQL_SAFE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^'\\]*$").expect("SQL_SAFE"));

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/distribution_point_post_codes/",
            get(get_current).post(post_create),
        )
        .route(
            "/distribution_point_post_codes/{business_key}/history",
            get(get_history),
        )
        .route(
            "/distribution_point_post_codes/{business_key}",
            put(put_update).delete(delete_one),
        )
}

#[derive(FromRow)]
struct PairingRow {
    row_id: String,
    distribution_point_post_code_id: String,
    dp: String,
    post_code: String,
    operation: String,
    valid_from: String,
    valid_to: Option<String>,
    created_by: Option<String>,
    created_at: Option<String>,
    updated_by: Option<String>,
    updated_at: Option<String>,
}

#[derive(Serialize)]
pub struct DistributionPointPostCodeModel {
    pub row_id: String,
    pub distribution_point_post_code_id: String,
    pub dp: String,
    pub post_code: String,
    pub operation: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub is_current: bool,
    pub created_by: Option<String>,
    pub created_at: Option<String>,
    pub updated_by: Option<String>,
    pub updated_at: Option<String>,
}

impl From<PairingRow> for DistributionPointPostCodeModel {
    fn from(r: PairingRow) -> Self {
        let is_current = r.valid_to.is_none() && r.operation != "DELETE";
        Self {
            row_id: r.row_id,
            distribution_point_post_code_id: r.distribution_point_post_code_id,
            dp: r.dp,
            post_code: r.post_code,
            operation: r.operation,
            valid_from: r.valid_from,
            valid_to: r.valid_to,
            is_current,
            created_by: r.created_by,
            created_at: r.created_at,
            updated_by: r.updated_by,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Deserialize)]
pub struct DistributionPointPostCodeInput {
    pub dp: String,
    pub post_code: String,
}

impl DistributionPointPostCodeInput {
    fn validate(&self) -> Result<(), ApiError> {
        for (name, val) in [("dp", &self.dp), ("post_code", &self.post_code)] {
            if val.is_empty() || val.len() > 32 || !SQL_SAFE.is_match(val) {
                return Err(ApiError::BadRequest(format!("invalid {name}: {val:?}")));
            }
        }
        Ok(())
    }

    fn business_key(&self) -> String {
        generate_hash_id(&[Some(&self.dp), Some(&self.post_code)])
    }
}

fn validate_business_key(key: &str) -> Result<(), ApiError> {
    if !SAFE_HEX_64.is_match(key) {
        return Err(ApiError::BadRequest(format!(
            "invalid business_key: {key:?}"
        )));
    }
    Ok(())
}

async fn get_current(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> ApiResult<Json<Vec<DistributionPointPostCodeModel>>> {
    let rows: Vec<PairingRow> = state
        .db
        .load_data(
            r#"
            SELECT
                row_id,
                distribution_point_post_code_id,
                dp,
                post_code,
                operation,
                valid_from::varchar as valid_from,
                valid_to::varchar as valid_to,
                created_by,
                created_at::varchar as created_at,
                updated_by,
                updated_at::varchar as updated_at
            FROM {schema}.lever_current_distribution_point_post_codes
            ORDER BY dp, post_code
        "#,
            &Params::new(),
        )
        .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn get_history(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(business_key): Path<String>,
) -> ApiResult<Json<Vec<DistributionPointPostCodeModel>>> {
    validate_business_key(&business_key)?;

    let mut params = Params::new();
    params.insert(
        "distribution_point_post_code_id".to_string(),
        BindValue::Text(business_key.clone()),
    );

    let rows: Vec<PairingRow> = state
        .db
        .load_data(
            r#"
            SELECT
                row_id,
                distribution_point_post_code_id,
                dp,
                post_code,
                operation,
                valid_from::varchar as valid_from,
                valid_to::varchar as valid_to,
                created_by,
                created_at::varchar as created_at,
                updated_by,
                updated_at::varchar as updated_at
            FROM {schema}.lever_distribution_point_post_codes_v3
            WHERE distribution_point_post_code_id = %(distribution_point_post_code_id)s
            ORDER BY valid_from ASC
        "#,
            &params,
        )
        .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn post_create(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(body): Json<Vec<DistributionPointPostCodeInput>>,
) -> ApiResult<impl IntoResponse> {
    if body.is_empty() {
        return Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({"status": "success"})),
        ));
    }
    for item in &body {
        item.validate()?;
    }

    let acted_at = Utc::now();
    let ids: Vec<String> = body.iter().map(|i| i.business_key()).collect();

    let mut params = Params::new();
    params.insert("ids".to_string(), BindValue::TextList(ids.clone()));

    let existing: Vec<(String,)> = state
        .db
        .load_data(
            r#"
            SELECT distribution_point_post_code_id
            FROM {schema}.lever_current_distribution_point_post_codes
            WHERE distribution_point_post_code_id IN (%(ids)L)
            "#,
            &params,
        )
        .await?;
    if !existing.is_empty() {
        let colliding: Vec<String> = existing.into_iter().map(|t| t.0).collect();
        return Err(ApiError::Conflict(format!(
            "Business keys already have a current row: {colliding:?}"
        )));
    }

    insert_pairing_rows(&state, &body, &ids, "INSERT", &user.email, acted_at).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"status": "success"})),
    ))
}

async fn put_update(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Path(business_key): Path<String>,
    Json(body): Json<DistributionPointPostCodeInput>,
) -> ApiResult<Json<serde_json::Value>> {
    validate_business_key(&business_key)?;
    body.validate()?;

    let mut lookup_params = Params::new();
    lookup_params.insert(
        "distribution_point_post_code_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    let existing: Vec<(String,)> = state
        .db
        .load_data(
            r#"
            SELECT distribution_point_post_code_id
            FROM {schema}.lever_current_distribution_point_post_codes
            WHERE distribution_point_post_code_id = %(distribution_point_post_code_id)s
            "#,
            &lookup_params,
        )
        .await?;
    if existing.is_empty() {
        return Err(ApiError::NotFound(
            "No current distribution point post code for business_key".to_string(),
        ));
    }

    let acted_at = Utc::now();
    let mut update_params = Params::new();
    update_params.insert(
        "distribution_point_post_code_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    update_params.insert("actor".to_string(), BindValue::Text(user.email.clone()));
    update_params.insert("acted_at".to_string(), BindValue::DateTime(acted_at));

    state
        .db
        .execute_query(
            r#"
            UPDATE {schema}.lever_distribution_point_post_codes_v3
            SET valid_to = %(acted_at)s,
                updated_by = %(actor)s,
                updated_at = %(acted_at)s
            WHERE distribution_point_post_code_id = %(distribution_point_post_code_id)s
                AND valid_to IS NULL
                AND operation <> 'DELETE'
            "#,
            &update_params,
        )
        .await?;

    insert_pairing_rows(
        &state,
        std::slice::from_ref(&body),
        &[business_key.clone()],
        "UPDATE",
        &user.email,
        acted_at,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "distribution_point_post_code_id": business_key,
    })))
}

async fn delete_one(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Path(business_key): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    validate_business_key(&business_key)?;

    let mut lookup_params = Params::new();
    lookup_params.insert(
        "distribution_point_post_code_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    let existing: Vec<(String,)> = state
        .db
        .load_data(
            r#"
            SELECT distribution_point_post_code_id
            FROM {schema}.lever_current_distribution_point_post_codes
            WHERE distribution_point_post_code_id = %(distribution_point_post_code_id)s
            "#,
            &lookup_params,
        )
        .await?;
    if existing.is_empty() {
        return Err(ApiError::NotFound(
            "No current distribution point post code for business_key".to_string(),
        ));
    }

    let acted_at = Utc::now();
    let mut params = Params::new();
    params.insert(
        "distribution_point_post_code_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    params.insert("actor".to_string(), BindValue::Text(user.email));
    params.insert("acted_at".to_string(), BindValue::DateTime(acted_at));

    state
        .db
        .execute_query(
            r#"
            UPDATE {schema}.lever_distribution_point_post_codes_v3
            SET valid_to = %(acted_at)s,
                updated_by = %(actor)s,
                updated_at = %(acted_at)s,
                operation = 'DELETE'
            WHERE distribution_point_post_code_id = %(distribution_point_post_code_id)s
                AND valid_to IS NULL
                AND operation <> 'DELETE'
            "#,
            &params,
        )
        .await?;

    Ok(Json(serde_json::json!({"status": "success"})))
}

async fn insert_pairing_rows(
    state: &AppState,
    items: &[DistributionPointPostCodeInput],
    ids: &[String],
    operation: &str,
    actor: &str,
    acted_at: chrono::DateTime<Utc>,
) -> Result<(), ApiError> {
    let schema = state.db.schema();
    let cols_per_row: usize = 11;
    let placeholder_chunks: Vec<String> = (0..items.len())
        .map(|i| {
            let base = i * cols_per_row;
            let phs: Vec<String> = (1..=cols_per_row)
                .map(|j| format!("${}", base + j))
                .collect();
            format!("({})", phs.join(", "))
        })
        .collect();

    let sql = format!(
        r#"INSERT INTO {schema}.lever_distribution_point_post_codes_v3
            (row_id, distribution_point_post_code_id, dp, post_code,
             operation, valid_from, valid_to,
             created_by, created_at, updated_by, updated_at)
           VALUES {}"#,
        placeholder_chunks.join(", ")
    );

    let mut q = sqlx::query(&sql);
    for (item, id) in items.iter().zip(ids.iter()) {
        let row_id = Uuid::new_v4().simple().to_string();
        q = q.bind(row_id);
        q = q.bind(id);
        q = q.bind(item.dp.clone());
        q = q.bind(item.post_code.clone());
        q = q.bind(operation);
        q = q.bind(acted_at);
        q = q.bind(None::<chrono::DateTime<Utc>>);
        q = q.bind(actor);
        q = q.bind(acted_at);
        q = q.bind(None::<String>);
        q = q.bind(None::<chrono::DateTime<Utc>>);
    }
    q.execute(state.db.pool()).await?;
    Ok(())
}
