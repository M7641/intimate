//! SCD2 CRUD for mixed-pack formats.
//! Port of `modules/planner/levers/backend/routes/mixed_pack_formats/route.py`.

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
        .route("/mixed_pack_formats/", get(get_current).post(post_create))
        .route(
            "/mixed_pack_formats/{business_key}/history",
            get(get_history),
        )
        .route(
            "/mixed_pack_formats/{business_key}",
            put(put_update).delete(delete_one),
        )
}

#[derive(FromRow)]
struct FormatRow {
    row_id: String,
    mixed_pack_format_id: String,
    from_deldate: Option<String>,
    to_deldate: Option<String>,
    hocustcode: Option<String>,
    prodnum: Option<String>,
    dp: Option<String>,
    brand: Option<String>,
    stra_ratio: Option<f64>,
    rasp_ratio: Option<f64>,
    blue_ratio: Option<f64>,
    blac_ratio: Option<f64>,
    operation: String,
    valid_from: String,
    valid_to: Option<String>,
    created_by: Option<String>,
    created_at: Option<String>,
    updated_by: Option<String>,
    updated_at: Option<String>,
}

#[derive(Serialize)]
pub struct MixedPackFormatModel {
    pub row_id: String,
    pub mixed_pack_format_id: String,
    pub from_deldate: Option<String>,
    pub to_deldate: Option<String>,
    pub hocustcode: Option<String>,
    pub prodnum: Option<String>,
    pub dp: Option<String>,
    pub brand: Option<String>,
    pub stra_ratio: Option<f64>,
    pub rasp_ratio: Option<f64>,
    pub blue_ratio: Option<f64>,
    pub blac_ratio: Option<f64>,
    pub operation: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub is_current: bool,
    pub created_by: Option<String>,
    pub created_at: Option<String>,
    pub updated_by: Option<String>,
    pub updated_at: Option<String>,
    pub active: i32,
}

impl From<FormatRow> for MixedPackFormatModel {
    fn from(r: FormatRow) -> Self {
        let is_current = r.valid_to.is_none() && r.operation != "DELETE";
        Self {
            row_id: r.row_id,
            mixed_pack_format_id: r.mixed_pack_format_id,
            from_deldate: r.from_deldate,
            to_deldate: r.to_deldate,
            hocustcode: r.hocustcode,
            prodnum: r.prodnum,
            dp: r.dp,
            brand: r.brand,
            stra_ratio: r.stra_ratio,
            rasp_ratio: r.rasp_ratio,
            blue_ratio: r.blue_ratio,
            blac_ratio: r.blac_ratio,
            operation: r.operation,
            valid_from: r.valid_from,
            valid_to: r.valid_to,
            is_current,
            created_by: r.created_by,
            created_at: r.created_at,
            updated_by: r.updated_by,
            updated_at: r.updated_at,
            active: 1,
        }
    }
}

#[derive(Deserialize)]
pub struct MixedPackFormatInput {
    pub from_deldate: String,
    pub to_deldate: String,
    pub hocustcode: String,
    pub prodnum: String,
    pub dp: String,
    pub brand: Option<String>,
    pub stra_ratio: f64,
    pub rasp_ratio: f64,
    pub blue_ratio: f64,
    pub blac_ratio: f64,
}

impl MixedPackFormatInput {
    fn validate(&self) -> Result<(), ApiError> {
        for (name, val) in [
            ("from_deldate", &self.from_deldate),
            ("to_deldate", &self.to_deldate),
            ("hocustcode", &self.hocustcode),
            ("prodnum", &self.prodnum),
            ("dp", &self.dp),
        ] {
            if val.is_empty() || val.len() > 32 || !SQL_SAFE.is_match(val) {
                return Err(ApiError::BadRequest(format!("invalid {name}: {val:?}")));
            }
        }
        if let Some(b) = &self.brand {
            if b.len() > 32 || !SQL_SAFE.is_match(b) {
                return Err(ApiError::BadRequest(format!("invalid brand: {b:?}")));
            }
        }
        for (name, ratio) in [
            ("stra_ratio", self.stra_ratio),
            ("rasp_ratio", self.rasp_ratio),
            ("blue_ratio", self.blue_ratio),
            ("blac_ratio", self.blac_ratio),
        ] {
            if !ratio.is_finite() || ratio < 0.0 {
                return Err(ApiError::BadRequest(format!(
                    "invalid {name}: must be finite and >= 0"
                )));
            }
        }
        Ok(())
    }

    fn business_key(&self) -> String {
        generate_hash_id(&[
            Some(&self.from_deldate),
            Some(&self.to_deldate),
            Some(&self.hocustcode),
            Some(&self.prodnum),
            Some(&self.dp),
        ])
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
) -> ApiResult<Json<Vec<MixedPackFormatModel>>> {
    let rows: Vec<FormatRow> = state
        .db
        .load_data(
            r#"
            SELECT
                row_id,
                mixed_pack_format_id,
                from_deldate,
                to_deldate,
                hocustcode,
                prodnum,
                dp,
                brand,
                stra_ratio,
                rasp_ratio,
                blue_ratio,
                blac_ratio,
                operation,
                valid_from::varchar as valid_from,
                valid_to::varchar as valid_to,
                created_by,
                created_at::varchar as created_at,
                updated_by,
                updated_at::varchar as updated_at
            FROM {schema}.lever_current_mixed_pack_formats
            ORDER BY hocustcode, prodnum, dp
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
) -> ApiResult<Json<Vec<MixedPackFormatModel>>> {
    validate_business_key(&business_key)?;

    let mut params = Params::new();
    params.insert(
        "mixed_pack_format_id".to_string(),
        BindValue::Text(business_key.clone()),
    );

    let rows: Vec<FormatRow> = state
        .db
        .load_data(
            r#"
            SELECT
                row_id,
                mixed_pack_format_id,
                from_deldate,
                to_deldate,
                hocustcode,
                prodnum,
                dp,
                brand,
                stra_ratio,
                rasp_ratio,
                blue_ratio,
                blac_ratio,
                operation,
                valid_from::varchar as valid_from,
                valid_to::varchar as valid_to,
                created_by,
                created_at::varchar as created_at,
                updated_by,
                updated_at::varchar as updated_at
            FROM {schema}.lever_mixed_pack_formats_v3
            WHERE mixed_pack_format_id = %(mixed_pack_format_id)s
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
    Json(body): Json<Vec<MixedPackFormatInput>>,
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
            SELECT mixed_pack_format_id
            FROM {schema}.lever_current_mixed_pack_formats
            WHERE mixed_pack_format_id IN (%(ids)L)
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

    insert_format_rows(&state, &body, &ids, "INSERT", &user.email, acted_at).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"status": "success"})),
    ))
}

async fn put_update(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Path(business_key): Path<String>,
    Json(body): Json<MixedPackFormatInput>,
) -> ApiResult<Json<serde_json::Value>> {
    validate_business_key(&business_key)?;
    body.validate()?;

    let mut lookup_params = Params::new();
    lookup_params.insert(
        "mixed_pack_format_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    let existing: Vec<(String,)> = state
        .db
        .load_data(
            r#"
            SELECT mixed_pack_format_id
            FROM {schema}.lever_current_mixed_pack_formats
            WHERE mixed_pack_format_id = %(mixed_pack_format_id)s
            "#,
            &lookup_params,
        )
        .await?;
    if existing.is_empty() {
        return Err(ApiError::NotFound(
            "No current mixed pack format for business_key".to_string(),
        ));
    }

    let acted_at = Utc::now();
    let mut update_params = Params::new();
    update_params.insert(
        "mixed_pack_format_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    update_params.insert("actor".to_string(), BindValue::Text(user.email.clone()));
    update_params.insert("acted_at".to_string(), BindValue::DateTime(acted_at));

    state
        .db
        .execute_query(
            r#"
            UPDATE {schema}.lever_mixed_pack_formats_v3
            SET valid_to = %(acted_at)s,
                updated_by = %(actor)s,
                updated_at = %(acted_at)s
            WHERE mixed_pack_format_id = %(mixed_pack_format_id)s
                AND valid_to IS NULL
                AND operation <> 'DELETE'
            "#,
            &update_params,
        )
        .await?;

    insert_format_rows(
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
        "mixed_pack_format_id": business_key,
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
        "mixed_pack_format_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    let existing: Vec<(String,)> = state
        .db
        .load_data(
            r#"
            SELECT mixed_pack_format_id
            FROM {schema}.lever_current_mixed_pack_formats
            WHERE mixed_pack_format_id = %(mixed_pack_format_id)s
            "#,
            &lookup_params,
        )
        .await?;
    if existing.is_empty() {
        return Err(ApiError::NotFound(
            "No current mixed pack format for business_key".to_string(),
        ));
    }

    let acted_at = Utc::now();
    let mut params = Params::new();
    params.insert(
        "mixed_pack_format_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    params.insert("actor".to_string(), BindValue::Text(user.email));
    params.insert("acted_at".to_string(), BindValue::DateTime(acted_at));

    state
        .db
        .execute_query(
            r#"
            UPDATE {schema}.lever_mixed_pack_formats_v3
            SET valid_to = %(acted_at)s,
                updated_by = %(actor)s,
                updated_at = %(acted_at)s,
                operation = 'DELETE'
            WHERE mixed_pack_format_id = %(mixed_pack_format_id)s
                AND valid_to IS NULL
                AND operation <> 'DELETE'
            "#,
            &params,
        )
        .await?;

    Ok(Json(serde_json::json!({"status": "success"})))
}

async fn insert_format_rows(
    state: &AppState,
    items: &[MixedPackFormatInput],
    ids: &[String],
    operation: &str,
    actor: &str,
    acted_at: chrono::DateTime<Utc>,
) -> Result<(), ApiError> {
    let schema = state.db.schema();
    let cols_per_row: usize = 19;
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
        r#"INSERT INTO {schema}.lever_mixed_pack_formats_v3
            (row_id, mixed_pack_format_id,
             from_deldate, to_deldate, hocustcode, prodnum, dp, brand,
             stra_ratio, rasp_ratio, blue_ratio, blac_ratio,
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
        q = q.bind(item.from_deldate.clone());
        q = q.bind(item.to_deldate.clone());
        q = q.bind(item.hocustcode.clone());
        q = q.bind(item.prodnum.clone());
        q = q.bind(item.dp.clone());
        q = q.bind(item.brand.clone().unwrap_or_default());
        q = q.bind(item.stra_ratio);
        q = q.bind(item.rasp_ratio);
        q = q.bind(item.blue_ratio);
        q = q.bind(item.blac_ratio);
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
