//! Approval-cube SCD2 CRUD: weekly's tweaks layer on top of `daily_approval_cube_all`.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
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

use common_rs::db::{BindValue, Params};
use common_rs::hash::generate_hash_id;
use common_rs::validation::SAFE_HEX_64;

use crate::state::AppState;
use common_rs::auth::AuthenticatedUser;
use common_rs::error::{ApiError, ApiResult};

// Block characters that can break out of a single-quoted SQL string literal.
static SQL_SAFE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^'\\]*$").expect("SQL_SAFE"));

static ISO_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}([T ][\d:.]+Z?)?$").expect("ISO_DATE"));

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/approval_cube/base_data", get(get_base_data))
        .route(
            "/approval_cube/available_options",
            get(get_available_options),
        )
        .route("/approval_cube/", get(get_current).post(post_create))
        .route("/approval_cube/{business_key}/history", get(get_history))
        .route(
            "/approval_cube/{business_key}",
            put(put_update).delete(delete_one),
        )
}

#[derive(Serialize, FromRow)]
pub struct ApprovedOption {
    pub supcode: String,
    pub variety: String,
}

#[derive(Serialize, FromRow)]
pub struct ApprovalCubeBaseRow {
    pub supcode: Option<String>,
    pub mascode: Option<String>,
    pub variety: Option<String>,
    pub hocustcode: Option<String>,
    pub brand: Option<String>,
    pub approval_cube_adjustment_id: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub active: i32,
}

#[derive(FromRow)]
struct AdjustmentRow {
    row_id: String,
    approval_cube_adjustment_id: String,
    supcode: Option<String>,
    mascode: Option<String>,
    variety: Option<String>,
    hocustcode: Option<String>,
    brand: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    active: i32,
    operation: String,
    valid_from: String,
    valid_to: Option<String>,
    created_by: Option<String>,
    created_at: Option<String>,
    updated_by: Option<String>,
    updated_at: Option<String>,
}

#[derive(Serialize)]
pub struct ApprovalCubeAdjustmentModel {
    pub row_id: String,
    pub approval_cube_adjustment_id: String,
    pub supcode: Option<String>,
    pub mascode: Option<String>,
    pub variety: Option<String>,
    pub hocustcode: Option<String>,
    pub brand: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub active: i32,
    pub operation: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub is_current: bool,
    pub created_by: Option<String>,
    pub created_at: Option<String>,
    pub updated_by: Option<String>,
    pub updated_at: Option<String>,
}

impl From<AdjustmentRow> for ApprovalCubeAdjustmentModel {
    fn from(r: AdjustmentRow) -> Self {
        let is_current = r.valid_to.is_none() && r.operation != "DELETE";
        Self {
            row_id: r.row_id,
            approval_cube_adjustment_id: r.approval_cube_adjustment_id,
            supcode: r.supcode,
            mascode: r.mascode,
            variety: r.variety,
            hocustcode: r.hocustcode,
            brand: r.brand,
            date_from: r.date_from,
            date_to: r.date_to,
            active: r.active,
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
pub struct ApprovalCubeAdjustmentInput {
    pub supcode: Option<String>,
    pub mascode: Option<String>,
    pub variety: Option<String>,
    pub hocustcode: Option<String>,
    pub brand: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    #[serde(default = "default_active")]
    pub active: i32,
}

fn default_active() -> i32 {
    1
}

impl ApprovalCubeAdjustmentInput {
    fn validate(&self) -> Result<(), ApiError> {
        for (name, val, max) in [
            ("supcode", &self.supcode, 16usize),
            ("mascode", &self.mascode, 16),
            ("variety", &self.variety, 32),
            ("hocustcode", &self.hocustcode, 32),
            ("brand", &self.brand, 32),
        ] {
            if let Some(v) = val {
                if v.len() > max || !SQL_SAFE.is_match(v) {
                    return Err(ApiError::BadRequest(format!("invalid {name}: {v:?}")));
                }
            }
        }
        for (name, val) in [("date_from", &self.date_from), ("date_to", &self.date_to)] {
            if let Some(v) = val {
                if !ISO_DATE.is_match(v) {
                    return Err(ApiError::BadRequest(format!("invalid {name}: {v:?}")));
                }
            }
        }
        if self.active != 0 && self.active != 1 {
            return Err(ApiError::BadRequest("active must be 0 or 1".to_string()));
        }
        Ok(())
    }

    fn business_key(&self) -> String {
        generate_hash_id(&[
            self.supcode.as_deref(),
            self.mascode.as_deref(),
            self.variety.as_deref(),
            self.hocustcode.as_deref(),
            self.brand.as_deref(),
        ])
    }
}

#[derive(Deserialize)]
pub struct AvailableOptionsQuery {
    pub hocustcode: String,
    pub mascode: String,
    pub brand: String,
}

fn validate_business_key(key: &str) -> Result<(), ApiError> {
    if !SAFE_HEX_64.is_match(key) {
        return Err(ApiError::BadRequest(format!(
            "invalid business_key: {key:?}"
        )));
    }
    Ok(())
}

async fn get_base_data(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> ApiResult<Json<Vec<ApprovalCubeBaseRow>>> {
    let rows: Vec<ApprovalCubeBaseRow> = state
        .db
        .load_data(
            r#"
            WITH adjustments AS (
                SELECT
                    approval_cube_adjustment_id,
                    COALESCE(supcode, '')    AS supcode,
                    COALESCE(mascode, '')    AS mascode,
                    COALESCE(variety, '')    AS variety,
                    COALESCE(hocustcode, '') AS hocustcode,
                    COALESCE(brand, '')      AS brand,
                    date_from::varchar       AS date_from,
                    date_to::varchar         AS date_to,
                    active
                FROM {schema}.weekly_current_approval_cube_adjustments
            )
            SELECT
                cube.supcode,
                cube.mascode,
                cube.variety,
                cube.hocustcode,
                cube.brand,
                adj.approval_cube_adjustment_id,
                adj.date_from,
                adj.date_to,
                COALESCE(adj.active, 0) AS active
            FROM {schema}.daily_approval_cube_all cube
            LEFT JOIN adjustments adj
                ON cube.supcode = adj.supcode
                AND cube.mascode = adj.mascode
                AND cube.variety = adj.variety
                AND cube.hocustcode = adj.hocustcode
                AND cube.brand = adj.brand
            ORDER BY active DESC
        "#,
            &Params::new(),
        )
        .await?;
    Ok(Json(rows))
}

async fn get_available_options(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Query(q): Query<AvailableOptionsQuery>,
) -> ApiResult<Json<Vec<ApprovedOption>>> {
    if !SQL_SAFE.is_match(&q.hocustcode) {
        return Err(ApiError::BadRequest(format!(
            "invalid hocustcode: {:?}",
            q.hocustcode
        )));
    }
    if !SQL_SAFE.is_match(&q.mascode) {
        return Err(ApiError::BadRequest(format!(
            "invalid mascode: {:?}",
            q.mascode
        )));
    }
    if !SQL_SAFE.is_match(&q.brand) {
        return Err(ApiError::BadRequest(format!(
            "invalid brand: {:?}",
            q.brand
        )));
    }

    let mut params = Params::new();

    let rows: Vec<ApprovedOption> = if q.mascode == "MIXB" {
        params.insert(
            "hocustcode".to_string(),
            BindValue::Text(q.hocustcode.clone()),
        );
        state
            .db
            .load_data(
                r#"
                WITH mixed_pack_active AS (
                    SELECT *
                    FROM {schema}.lever_current_mixed_pack_formats
                    WHERE hocustcode = %(hocustcode)s
                        AND to_deldate >= current_date
                        AND from_deldate <= current_date
                ),
                mixed_mascodes AS (
                    SELECT 'STRA' AS mascode FROM mixed_pack_active WHERE stra_ratio > 0
                    UNION SELECT 'RASP' FROM mixed_pack_active WHERE rasp_ratio > 0
                    UNION SELECT 'BLUE' FROM mixed_pack_active WHERE blue_ratio > 0
                    UNION SELECT 'BLAC' FROM mixed_pack_active WHERE blac_ratio > 0
                )
                SELECT DISTINCT cube.supcode, cube.variety
                FROM {schema}.daily_approval_cube_all cube
                INNER JOIN mixed_mascodes mm ON cube.mascode = mm.mascode
                WHERE cube.hocustcode = %(hocustcode)s
                    AND cube.brand = 'MIXED'
                ORDER BY cube.supcode, cube.variety
            "#,
                &params,
            )
            .await?
    } else {
        params.insert(
            "hocustcode".to_string(),
            BindValue::Text(q.hocustcode.clone()),
        );
        params.insert("mascode".to_string(), BindValue::Text(q.mascode.clone()));
        params.insert("brand".to_string(), BindValue::Text(q.brand.clone()));
        state
            .db
            .load_data(
                r#"
                SELECT DISTINCT supcode, variety
                FROM {schema}.daily_approval_cube_all
                WHERE hocustcode = %(hocustcode)s
                    AND mascode = %(mascode)s
                    AND brand = %(brand)s
                ORDER BY supcode, variety
            "#,
                &params,
            )
            .await?
    };
    Ok(Json(rows))
}

async fn get_current(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> ApiResult<Json<Vec<ApprovalCubeAdjustmentModel>>> {
    let rows: Vec<AdjustmentRow> = state
        .db
        .load_data(
            r#"
            SELECT
                row_id,
                approval_cube_adjustment_id,
                supcode, mascode, variety, hocustcode, brand,
                date_from::varchar as date_from,
                date_to::varchar as date_to,
                active, operation,
                valid_from::varchar as valid_from,
                valid_to::varchar as valid_to,
                created_by,
                created_at::varchar as created_at,
                updated_by,
                updated_at::varchar as updated_at
            FROM {schema}.weekly_current_approval_cube_adjustments
            WHERE active = 1
              AND date_from::timestamp <= current_timestamp
              AND date_to::timestamp >= current_timestamp
            ORDER BY date_from DESC
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
) -> ApiResult<Json<Vec<ApprovalCubeAdjustmentModel>>> {
    validate_business_key(&business_key)?;

    let mut params = Params::new();
    params.insert(
        "approval_cube_adjustment_id".to_string(),
        BindValue::Text(business_key.clone()),
    );

    let rows: Vec<AdjustmentRow> = state
        .db
        .load_data(
            r#"
            SELECT
                row_id, approval_cube_adjustment_id,
                supcode, mascode, variety, hocustcode, brand,
                date_from::varchar as date_from,
                date_to::varchar as date_to,
                active, operation,
                valid_from::varchar as valid_from,
                valid_to::varchar as valid_to,
                created_by,
                created_at::varchar as created_at,
                updated_by,
                updated_at::varchar as updated_at
            FROM {schema}.weekly_approval_cube_adjustments_v3
            WHERE approval_cube_adjustment_id = %(approval_cube_adjustment_id)s
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
    Json(body): Json<Vec<ApprovalCubeAdjustmentInput>>,
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

    let mut check_params = Params::new();
    check_params.insert("ids".to_string(), BindValue::TextList(ids.clone()));
    let existing: Vec<(String,)> = state
        .db
        .load_data(
            r#"
            SELECT approval_cube_adjustment_id
            FROM {schema}.weekly_current_approval_cube_adjustments
            WHERE approval_cube_adjustment_id IN (%(ids)L)
            "#,
            &check_params,
        )
        .await?;
    if !existing.is_empty() {
        let colliding: Vec<String> = existing.into_iter().map(|t| t.0).collect();
        return Err(ApiError::Conflict(format!(
            "Business keys already have a current row: {colliding:?}"
        )));
    }

    insert_adjustment_rows(&state, &body, &ids, "INSERT", &user.email, acted_at).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"status": "success"})),
    ))
}

async fn put_update(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Path(business_key): Path<String>,
    Json(body): Json<ApprovalCubeAdjustmentInput>,
) -> ApiResult<Json<serde_json::Value>> {
    validate_business_key(&business_key)?;
    body.validate()?;
    if body.business_key() != business_key {
        return Err(ApiError::BadRequest(
            "Payload identity hash does not match URL business_key".to_string(),
        ));
    }

    let mut lookup_params = Params::new();
    lookup_params.insert(
        "approval_cube_adjustment_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    let existing: Vec<(String,)> = state
        .db
        .load_data(
            r#"
            SELECT approval_cube_adjustment_id
            FROM {schema}.weekly_current_approval_cube_adjustments
            WHERE approval_cube_adjustment_id = %(approval_cube_adjustment_id)s
            "#,
            &lookup_params,
        )
        .await?;
    if existing.is_empty() {
        return Err(ApiError::NotFound(
            "No current approval cube adjustment for business_key".to_string(),
        ));
    }

    let acted_at = Utc::now();
    let mut update_params = Params::new();
    update_params.insert(
        "approval_cube_adjustment_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    update_params.insert("actor".to_string(), BindValue::Text(user.email.clone()));
    update_params.insert("acted_at".to_string(), BindValue::DateTime(acted_at));

    state
        .db
        .execute_query(
            r#"
            UPDATE {schema}.weekly_approval_cube_adjustments_v3
            SET valid_to = %(acted_at)s,
                updated_by = %(actor)s,
                updated_at = %(acted_at)s
            WHERE approval_cube_adjustment_id = %(approval_cube_adjustment_id)s
                AND valid_to IS NULL
                AND operation <> 'DELETE'
            "#,
            &update_params,
        )
        .await?;

    insert_adjustment_rows(
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
        "approval_cube_adjustment_id": business_key,
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
        "approval_cube_adjustment_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    let existing: Vec<(String,)> = state
        .db
        .load_data(
            r#"
            SELECT approval_cube_adjustment_id
            FROM {schema}.weekly_current_approval_cube_adjustments
            WHERE approval_cube_adjustment_id = %(approval_cube_adjustment_id)s
            "#,
            &lookup_params,
        )
        .await?;
    if existing.is_empty() {
        return Err(ApiError::NotFound(
            "No current approval cube adjustment for business_key".to_string(),
        ));
    }

    let acted_at = Utc::now();
    let mut params = Params::new();
    params.insert(
        "approval_cube_adjustment_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    params.insert("actor".to_string(), BindValue::Text(user.email));
    params.insert("acted_at".to_string(), BindValue::DateTime(acted_at));

    state
        .db
        .execute_query(
            r#"
            UPDATE {schema}.weekly_approval_cube_adjustments_v3
            SET valid_to = %(acted_at)s,
                updated_by = %(actor)s,
                updated_at = %(acted_at)s,
                operation = 'DELETE'
            WHERE approval_cube_adjustment_id = %(approval_cube_adjustment_id)s
                AND valid_to IS NULL
                AND operation <> 'DELETE'
            "#,
            &params,
        )
        .await?;

    Ok(Json(serde_json::json!({"status": "success"})))
}

async fn insert_adjustment_rows(
    state: &AppState,
    items: &[ApprovalCubeAdjustmentInput],
    ids: &[String],
    operation: &str,
    actor: &str,
    acted_at: chrono::DateTime<Utc>,
) -> Result<(), ApiError> {
    let schema = state.db.schema();
    let sql = format!(
        r#"INSERT INTO {schema}.weekly_approval_cube_adjustments_v3
            (row_id, approval_cube_adjustment_id, supcode, mascode, variety, hocustcode, brand,
             date_from, date_to, active, operation, valid_from, valid_to,
             created_by, created_at, updated_by, updated_at)
           VALUES "#
    );
    let mut sql = sql;
    let mut binds_per_row = 0;

    let placeholder_chunks: Vec<String> = (0..items.len())
        .map(|i| {
            let base = i * 17;
            binds_per_row = 17;
            let phs: Vec<String> = (1..=17).map(|j| format!("${}", base + j)).collect();
            format!("({})", phs.join(", "))
        })
        .collect();
    sql.push_str(&placeholder_chunks.join(", "));
    let _ = binds_per_row;

    let mut q = sqlx::query(&sql);
    for (item, id) in items.iter().zip(ids.iter()) {
        let row_id = Uuid::new_v4().simple().to_string();
        q = q.bind(row_id);
        q = q.bind(id);
        q = q.bind(item.supcode.clone());
        q = q.bind(item.mascode.clone());
        q = q.bind(item.variety.clone());
        q = q.bind(item.hocustcode.clone());
        q = q.bind(item.brand.clone());
        q = q.bind(item.date_from.clone());
        q = q.bind(item.date_to.clone());
        q = q.bind(item.active);
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
