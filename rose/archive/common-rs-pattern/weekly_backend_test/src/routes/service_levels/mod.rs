//! Weekly target service-levels SCD2 CRUD plus cascade dropdowns (with caching).

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use common_rs::db::{BindValue, Db, Params};
use common_rs::hash::generate_hash_id;
use common_rs::validation::{SAFE_CODE, SAFE_HEX_64};

use crate::state::AppState;
use common_rs::auth::AuthenticatedUser;
use common_rs::error::{ApiError, ApiResult};

const CASCADE_KEY_PREFIX: &str = "weekly:service_levels:cascade:";

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/weekly_service_levels/",
            get(get_current).post(post_create),
        )
        .route(
            "/weekly_service_levels/{business_key}/history",
            get(get_history),
        )
        .route(
            "/weekly_service_levels/{business_key}",
            put(put_upsert).delete(delete_one),
        )
        .route(
            "/weekly_service_levels/cascade/mascodes",
            get(cascade_mascodes),
        )
        .route(
            "/weekly_service_levels/cascade/hocustcodes",
            get(cascade_hocustcodes),
        )
        .route(
            "/weekly_service_levels/cascade/supcodes",
            get(cascade_supcodes),
        )
        .route("/weekly_service_levels/cascade/tiers", get(cascade_tiers))
}

#[derive(FromRow)]
struct ServiceLevelRow {
    row_id: String,
    service_level_id: String,
    service_level_name: String,
    supcode: Option<String>,
    hocustcode: Option<String>,
    mascode: String,
    tier: Option<String>,
    target_sense: String,
    target_service_level: i32,
    operation: String,
    valid_from: String,
    valid_to: Option<String>,
    created_by: Option<String>,
    created_at: Option<String>,
    updated_by: Option<String>,
    updated_at: Option<String>,
}

#[derive(Serialize)]
pub struct ServiceLevelModel {
    pub row_id: String,
    pub service_level_id: String,
    pub service_level_name: String,
    pub supcode: Option<String>,
    pub hocustcode: Option<String>,
    pub mascode: String,
    pub tier: Option<String>,
    pub target_sense: String,
    pub target_service_level: i32,
    pub operation: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub is_current: bool,
    pub created_by: Option<String>,
    pub created_at: Option<String>,
    pub updated_by: Option<String>,
    pub updated_at: Option<String>,
}

impl From<ServiceLevelRow> for ServiceLevelModel {
    fn from(r: ServiceLevelRow) -> Self {
        let is_current = r.valid_to.is_none() && r.operation != "DELETE";
        Self {
            row_id: r.row_id,
            service_level_id: r.service_level_id,
            service_level_name: r.service_level_name,
            supcode: r.supcode,
            hocustcode: r.hocustcode,
            mascode: r.mascode,
            tier: r.tier,
            target_sense: r.target_sense,
            target_service_level: r.target_service_level,
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
pub struct ServiceLevelInput {
    pub service_level_name: String,
    pub supcode: Option<String>,
    pub hocustcode: Option<String>,
    pub mascode: String,
    pub tier: Option<String>,
    pub target_sense: String,
    pub target_service_level: i32,
}

impl ServiceLevelInput {
    fn validate(&self) -> Result<(), ApiError> {
        match self.service_level_name.as_str() {
            "supcode_mascode" => {
                if self.supcode.as_deref().unwrap_or("").is_empty() {
                    return Err(ApiError::BadRequest(
                        "supcode is required for service_level_name='supcode_mascode'".to_string(),
                    ));
                }
            }
            "hocustcode_mascode" => {
                if self.hocustcode.as_deref().unwrap_or("").is_empty() {
                    return Err(ApiError::BadRequest(
                        "hocustcode is required for service_level_name='hocustcode_mascode'"
                            .to_string(),
                    ));
                }
            }
            "hocustcode_tier_mascode" => {
                if self.hocustcode.as_deref().unwrap_or("").is_empty() {
                    return Err(ApiError::BadRequest(
                        "hocustcode is required for service_level_name='hocustcode_tier_mascode'"
                            .to_string(),
                    ));
                }
                let t = self.tier.as_deref().unwrap_or("");
                if t.is_empty() || t == "0" {
                    return Err(ApiError::BadRequest(
                        "tier is required (and must not be '0') for service_level_name='hocustcode_tier_mascode'".to_string(),
                    ));
                }
            }
            other => {
                return Err(ApiError::BadRequest(format!(
                    "invalid service_level_name: {other:?}"
                )));
            }
        }

        for (name, val, max) in [
            ("supcode", &self.supcode, 16usize),
            ("hocustcode", &self.hocustcode, 16),
            ("tier", &self.tier, 32),
        ] {
            if let Some(v) = val {
                if v.len() > max || !SAFE_CODE.is_match(v) {
                    return Err(ApiError::BadRequest(format!("invalid {name}: {v:?}")));
                }
            }
        }
        if self.mascode.len() > 16 || !SAFE_CODE.is_match(&self.mascode) {
            return Err(ApiError::BadRequest(format!(
                "invalid mascode: {:?}",
                self.mascode
            )));
        }
        if self.target_sense != "at-least" && self.target_sense != "at-most" {
            return Err(ApiError::BadRequest(format!(
                "target_sense must be 'at-least' or 'at-most': {:?}",
                self.target_sense
            )));
        }
        if !(0..=100).contains(&self.target_service_level) {
            return Err(ApiError::BadRequest(
                "target_service_level must be in 0..=100".to_string(),
            ));
        }
        Ok(())
    }

    fn business_key(&self) -> String {
        generate_hash_id(&[
            Some(self.service_level_name.as_str()),
            Some(self.mascode.as_str()),
            self.supcode.as_deref(),
            self.hocustcode.as_deref(),
            self.tier.as_deref(),
            Some(self.target_sense.as_str()),
        ])
    }
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct MascodeOption {
    pub mascode: String,
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct HocustcodeOption {
    pub hocustcode: String,
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct SupcodeOption {
    pub supcode: String,
}

#[derive(Serialize)]
pub struct TierOption {
    pub tier: String,
}

#[derive(Deserialize)]
pub struct MascodeQueryParam {
    pub mascode: String,
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
) -> ApiResult<Json<Vec<ServiceLevelModel>>> {
    let mut rows: Vec<ServiceLevelRow> = state
        .db
        .load_data(
            r#"
            SELECT row_id, service_level_id, service_level_name,
                supcode, hocustcode, mascode, tier, target_sense, target_service_level, operation,
                valid_from::varchar as valid_from,
                valid_to::varchar as valid_to,
                created_by,
                created_at::varchar as created_at,
                updated_by,
                updated_at::varchar as updated_at
            FROM {schema}.weekly_current_target_service_levels
        "#,
            &Params::new(),
        )
        .await?;
    rows.sort_by(|a, b| b.valid_from.cmp(&a.valid_from));
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn get_history(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(business_key): Path<String>,
) -> ApiResult<Json<Vec<ServiceLevelModel>>> {
    validate_business_key(&business_key)?;
    let mut params = Params::new();
    params.insert(
        "service_level_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    let rows: Vec<ServiceLevelRow> = state
        .db
        .load_data(
            r#"
            SELECT row_id, service_level_id, service_level_name,
                supcode, hocustcode, mascode, tier, target_sense, target_service_level, operation,
                valid_from::varchar as valid_from,
                valid_to::varchar as valid_to,
                created_by,
                created_at::varchar as created_at,
                updated_by,
                updated_at::varchar as updated_at
            FROM {schema}.weekly_target_service_levels
            WHERE service_level_id = %(service_level_id)s
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
    Json(body): Json<Vec<ServiceLevelInput>>,
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
    let ids: Vec<String> = body.iter().map(|i| i.business_key()).collect();

    let mut check_params = Params::new();
    check_params.insert("ids".to_string(), BindValue::TextList(ids.clone()));
    let existing: Vec<(String,)> = state
        .db
        .load_data(
            r#"
            SELECT service_level_id
            FROM {schema}.weekly_current_target_service_levels
            WHERE service_level_id IN (%(ids)L)
            "#,
            &check_params,
        )
        .await?;
    if !existing.is_empty() {
        let colliding: Vec<String> = existing.into_iter().map(|t| t.0).collect();
        return Err(ApiError::Conflict(format!(
            "Service level ids already have a current row: {colliding:?}"
        )));
    }

    let acted_at = Utc::now();
    insert_service_level_rows(&state, &body, &ids, "INSERT", &user.email, acted_at).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"status": "success"})),
    ))
}

async fn put_upsert(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Path(business_key): Path<String>,
    Json(body): Json<ServiceLevelInput>,
) -> ApiResult<Json<serde_json::Value>> {
    validate_business_key(&business_key)?;
    body.validate()?;
    if body.business_key() != business_key {
        return Err(ApiError::BadRequest(
            "business_key in URL does not match hash of payload identity fields".to_string(),
        ));
    }

    let mut lookup_params = Params::new();
    lookup_params.insert(
        "service_level_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    let existing: Vec<(String,)> = state
        .db
        .load_data(
            r#"
            SELECT service_level_id
            FROM {schema}.weekly_current_target_service_levels
            WHERE service_level_id = %(service_level_id)s
            "#,
            &lookup_params,
        )
        .await?;
    let has_current = !existing.is_empty();

    let acted_at = Utc::now();
    if has_current {
        let mut params = Params::new();
        params.insert(
            "service_level_id".to_string(),
            BindValue::Text(business_key.clone()),
        );
        params.insert("actor".to_string(), BindValue::Text(user.email.clone()));
        params.insert("acted_at".to_string(), BindValue::DateTime(acted_at));
        state
            .db
            .execute_query(
                r#"
                UPDATE {schema}.weekly_target_service_levels
                SET valid_to = %(acted_at)s,
                    updated_by = %(actor)s,
                    updated_at = %(acted_at)s
                WHERE service_level_id = %(service_level_id)s
                    AND valid_to IS NULL
                    AND operation <> 'DELETE'
                "#,
                &params,
            )
            .await?;
    }

    let op = if has_current { "UPDATE" } else { "INSERT" };
    insert_service_level_rows(
        &state,
        std::slice::from_ref(&body),
        &[business_key.clone()],
        op,
        &user.email,
        acted_at,
    )
    .await?;
    Ok(Json(serde_json::json!({"status": "success"})))
}

async fn delete_one(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Path(business_key): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    validate_business_key(&business_key)?;

    let mut lookup_params = Params::new();
    lookup_params.insert(
        "service_level_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    let existing: Vec<(String,)> = state
        .db
        .load_data(
            r#"
            SELECT service_level_id
            FROM {schema}.weekly_current_target_service_levels
            WHERE service_level_id = %(service_level_id)s
            "#,
            &lookup_params,
        )
        .await?;
    if existing.is_empty() {
        return Err(ApiError::NotFound(
            "No current service level for business_key".to_string(),
        ));
    }

    let acted_at = Utc::now();
    let mut params = Params::new();
    params.insert(
        "service_level_id".to_string(),
        BindValue::Text(business_key.clone()),
    );
    params.insert("actor".to_string(), BindValue::Text(user.email));
    params.insert("acted_at".to_string(), BindValue::DateTime(acted_at));

    state
        .db
        .execute_query(
            r#"
            UPDATE {schema}.weekly_target_service_levels
            SET valid_to = %(acted_at)s,
                updated_by = %(actor)s,
                updated_at = %(acted_at)s,
                operation = 'DELETE'
            WHERE service_level_id = %(service_level_id)s
                AND valid_to IS NULL
                AND operation <> 'DELETE'
            "#,
            &params,
        )
        .await?;
    Ok(Json(serde_json::json!({"status": "success"})))
}

// ── Cascade dropdowns ──────────────────────────────────────────────

async fn cascade_mascodes(State(state): State<AppState>) -> ApiResult<Json<Vec<MascodeOption>>> {
    let schema = state.db.schema();
    let key = format!("{CASCADE_KEY_PREFIX}mascodes:{schema}");
    let db: Db = state.db.clone();

    let value = state
        .cascade_cache
        .get_with(key, async move {
            let rows: Vec<MascodeOption> = db
                .load_data(
                    "SELECT mascode FROM {schema}.weekly_mascodes",
                    &Params::new(),
                )
                .await
                .unwrap_or_default();
            serde_json::to_value(rows).unwrap_or(serde_json::json!([]))
        })
        .await;
    let rows: Vec<MascodeOption> = serde_json::from_value(value).unwrap_or_default();
    Ok(Json(rows))
}

async fn cascade_hocustcodes(
    State(state): State<AppState>,
    Query(q): Query<MascodeQueryParam>,
) -> ApiResult<Json<Vec<HocustcodeOption>>> {
    if q.mascode.len() > 16 || !SAFE_CODE.is_match(&q.mascode) {
        return Err(ApiError::BadRequest(format!(
            "invalid mascode: {:?}",
            q.mascode
        )));
    }
    let schema = state.db.schema();
    let key = format!("{CASCADE_KEY_PREFIX}hocustcodes:{schema}:{}", q.mascode);
    let db: Db = state.db.clone();
    let mascode = q.mascode.clone();

    let value = state
        .cascade_cache
        .get_with(key, async move {
            let mut params = Params::new();
            params.insert("mascode".to_string(), BindValue::Text(mascode));
            let rows: Vec<HocustcodeOption> = db
                .load_data(
                    r#"
                    SELECT hocustcode FROM {schema}.weekly_customers
                    WHERE mascode = %(mascode)s ORDER BY hocustcode
                    "#,
                    &params,
                )
                .await
                .unwrap_or_default();
            serde_json::to_value(rows).unwrap_or(serde_json::json!([]))
        })
        .await;
    let rows: Vec<HocustcodeOption> = serde_json::from_value(value).unwrap_or_default();
    Ok(Json(rows))
}

async fn cascade_supcodes(
    State(state): State<AppState>,
    Query(q): Query<MascodeQueryParam>,
) -> ApiResult<Json<Vec<SupcodeOption>>> {
    if q.mascode.len() > 16 || !SAFE_CODE.is_match(&q.mascode) {
        return Err(ApiError::BadRequest(format!(
            "invalid mascode: {:?}",
            q.mascode
        )));
    }
    let schema = state.db.schema();
    let key = format!("{CASCADE_KEY_PREFIX}supcodes:{schema}:{}", q.mascode);
    let db: Db = state.db.clone();
    let mascode = q.mascode.clone();

    let value = state
        .cascade_cache
        .get_with(key, async move {
            let mut params = Params::new();
            params.insert("mascode".to_string(), BindValue::Text(mascode));
            let rows: Vec<SupcodeOption> = db
                .load_data(
                    r#"
                    SELECT supcode FROM {schema}.weekly_growers
                    WHERE mascode = %(mascode)s
                    "#,
                    &params,
                )
                .await
                .unwrap_or_default();
            serde_json::to_value(rows).unwrap_or(serde_json::json!([]))
        })
        .await;
    let rows: Vec<SupcodeOption> = serde_json::from_value(value).unwrap_or_default();
    Ok(Json(rows))
}

async fn cascade_tiers() -> Json<Vec<TierOption>> {
    Json(
        ["PREMIUM", "STANDARD", "VALUE"]
            .into_iter()
            .map(|t| TierOption {
                tier: t.to_string(),
            })
            .collect(),
    )
}

async fn insert_service_level_rows(
    state: &AppState,
    items: &[ServiceLevelInput],
    ids: &[String],
    operation: &str,
    actor: &str,
    acted_at: chrono::DateTime<Utc>,
) -> Result<(), ApiError> {
    let schema = state.db.schema();
    let mut sql = format!(
        r#"INSERT INTO {schema}.weekly_target_service_levels
            (row_id, service_level_id, service_level_name,
             supcode, hocustcode, mascode, tier, target_sense, target_service_level,
             operation, valid_from, valid_to,
             created_by, created_at, updated_by, updated_at)
           VALUES "#
    );
    let placeholder_chunks: Vec<String> = (0..items.len())
        .map(|i| {
            let base = i * 16;
            let phs: Vec<String> = (1..=16).map(|j| format!("${}", base + j)).collect();
            format!("({})", phs.join(", "))
        })
        .collect();
    sql.push_str(&placeholder_chunks.join(", "));

    let mut q = sqlx::query(&sql);
    for (item, id) in items.iter().zip(ids.iter()) {
        q = q.bind(Uuid::new_v4().simple().to_string());
        q = q.bind(id);
        q = q.bind(item.service_level_name.clone());
        q = q.bind(item.supcode.clone());
        q = q.bind(item.hocustcode.clone());
        q = q.bind(item.mascode.clone());
        q = q.bind(item.tier.clone());
        q = q.bind(item.target_sense.clone());
        q = q.bind(item.target_service_level);
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
