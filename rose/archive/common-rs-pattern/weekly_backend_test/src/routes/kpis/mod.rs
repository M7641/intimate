//! KPI dashboards. All endpoints accept the same four optional filters
//! (mascode, supcode, hocustcode, created_at). Each filter is always bound;
//! when the caller did not supply a value, it is bound as `NULL` and the
//! SQL clause `(%(x)s::varchar IS NULL OR col = %(x)s)` collapses to
//! "always true", so the same SQL works for every combination of filters.
//!
//! NOTE on column types: response structs are tailored to what each SQL
//! actually returns, not to the python Pydantic schema. Several queries
//! return aggregated shapes (min/max/avg, or pivoted by tier) and DO NOT
//! return mascode/entity_code/etc. The python side coerced missing
//! columns to None via Pydantic; sqlx is strict.

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use common_rs::db::{BindValue, Params};
use common_rs::validation::{SAFE_CODE, SAFE_DATE};

use crate::state::AppState;
use common_rs::error::{ApiError, ApiResult};

const CUMULATIVE_COMPLEXITY_SQL: &str = include_str!("sql/cumulative_complexity.sql");
const SUPPLIER_COMPLEXITY_SQL: &str = include_str!("sql/supplier_complexity.sql");
const CUSTOMER_COMPLEXITY_SQL: &str = include_str!("sql/customer_complexity.sql");
const CUSTOMER_SERVICE_LEVELS_SQL: &str = include_str!("sql/customer_service_levels.sql");
const SUPPLIER_SERVICE_LEVELS_SQL: &str = include_str!("sql/supplier_service_levels.sql");
const SUPPLIER_SERVICE_LEVELS_DETAILS_SQL: &str =
    include_str!("sql/supplier_service_levels_details.sql");
const SUPPLIER_TIER_SERVICE_LEVELS_SQL: &str = include_str!("sql/supplier_tier_service_levels.sql");
const SUPPLIER_SUMMARY_METRICS_SQL: &str = include_str!("sql/supplier_summary_metrics.sql");
const CUSTOMER_TIER_SERVICE_LEVELS_SQL: &str = include_str!("sql/customer_tier_service_levels.sql");
const CUSTOMER_SUMMARY_METRICS_SQL: &str = include_str!("sql/customer_summary_metrics.sql");
const SUPPLY_AND_DEMAND_SQL: &str = include_str!("sql/supply_and_demand.sql");
const SUPPLY_AND_DEMAND_DETAILS_SQL: &str = include_str!("sql/supply_and_demand_details.sql");
const ANNUAL_DEVIATION_SQL: &str = include_str!("sql/annual_deviation.sql");

const CUSTOMER_SERVICE_LEVEL_DETAILS_SQL: &str = r#"
    SELECT
        CAST(created_at AS DATE) AS created_at,
        hocustcode,
        CASE
            WHEN (demand_wgt - allocated_wgt) < 0 THEN 0
            ELSE ROUND(demand_wgt - allocated_wgt, 1)
        END AS shortage,
        allocated_wgt / demand_wgt AS service_level
    FROM {schema}.weekly_optimiser_output
    WHERE 1=1
    AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
    AND (%(supcode)s::varchar IS NULL OR supcode = %(supcode)s)
    AND (%(hocustcode)s::varchar IS NULL OR hocustcode = %(hocustcode)s)
    AND (%(created_at)s::varchar IS NULL OR created_at = %(created_at)s)
    ORDER BY created_at
"#;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kpis/run_timestamps", get(get_run_timestamps))
        .route(
            "/kpis/cumulative_complexity",
            get(get_cumulative_complexity),
        )
        .route("/kpis/supplier_complexity", get(get_supplier_complexity))
        .route("/kpis/customer_complexity", get(get_customer_complexity))
        .route(
            "/kpis/customer_service_levels",
            get(get_customer_service_levels),
        )
        .route(
            "/kpis/customer_service_level_details",
            get(get_customer_service_level_details),
        )
        .route(
            "/kpis/supplier_service_levels",
            get(get_supplier_service_levels),
        )
        .route(
            "/kpis/supplier_service_level_details",
            get(get_supplier_service_level_details),
        )
        .route(
            "/kpis/supplier_tier_service_levels",
            get(get_supplier_tier_service_levels),
        )
        .route(
            "/kpis/supplier_summary_metrics",
            get(get_supplier_summary_metrics),
        )
        .route(
            "/kpis/customer_tier_service_levels",
            get(get_customer_tier_service_levels),
        )
        .route(
            "/kpis/customer_summary_metrics",
            get(get_customer_summary_metrics),
        )
        .route("/kpis/supply_and_demand", get(get_supply_and_demand))
        .route(
            "/kpis/supply_and_demand_details",
            get(get_supply_and_demand_details),
        )
        .route("/kpis/annual_deviation", get(get_annual_deviation))
}

#[derive(Deserialize)]
pub struct KpiFilters {
    pub mascode: Option<String>,
    pub supcode: Option<String>,
    pub hocustcode: Option<String>,
    pub created_at: Option<String>,
}

impl KpiFilters {
    fn validate(&self) -> Result<(), ApiError> {
        for (name, val, max) in [
            ("mascode", &self.mascode, 16usize),
            ("supcode", &self.supcode, 16),
            ("hocustcode", &self.hocustcode, 32),
        ] {
            if let Some(v) = val {
                if v.len() > max || !SAFE_CODE.is_match(v) {
                    return Err(ApiError::BadRequest(format!("invalid {name}: {v:?}")));
                }
            }
        }
        if let Some(v) = &self.created_at {
            if v.len() > 32 || !SAFE_DATE.is_match(v) {
                return Err(ApiError::BadRequest(format!("invalid created_at: {v:?}")));
            }
        }
        Ok(())
    }

    fn to_params(&self) -> Params {
        let mut params = Params::new();
        params.insert("mascode".to_string(), opt_text(&self.mascode));
        params.insert("supcode".to_string(), opt_text(&self.supcode));
        params.insert("hocustcode".to_string(), opt_text(&self.hocustcode));
        params.insert("created_at".to_string(), opt_text(&self.created_at));
        params
    }
}

fn opt_text(v: &Option<String>) -> BindValue {
    match v {
        Some(s) => BindValue::Text(s.clone()),
        None => BindValue::Null,
    }
}

// ── Per-endpoint structs matching actual SQL output ──────────────────

#[derive(Serialize, FromRow)]
pub struct RunTimestamp {
    pub run_timestamp: String,
    pub run_date: String,
}

/// `cumulative_complexity.sql` returns `created_at, complexity_for_week`.
#[derive(Serialize, FromRow)]
pub struct CumulativeComplexityRow {
    pub created_at: Option<String>,
    pub complexity_for_week: Option<i64>,
}

/// `supplier_complexity.sql` returns `created_at` cast to DATE plus min/max/avg.
#[derive(Serialize, FromRow)]
pub struct SupplierComplexityRow {
    pub created_at: Option<NaiveDate>,
    pub min_complexity: Option<i64>,
    pub max_complexity: Option<i64>,
    pub average_complexity: Option<f64>,
}

/// `customer_complexity.sql` returns raw `created_at` (VARCHAR) plus min/max/avg.
#[derive(Serialize, FromRow)]
pub struct CustomerComplexityRow {
    pub created_at: Option<String>,
    pub min_complexity: Option<i64>,
    pub max_complexity: Option<i64>,
    pub average_complexity: Option<f64>,
}

/// `customer_service_levels.sql` returns `created_at` cast to DATE plus min/max/avg/std_dev.
#[derive(Serialize, FromRow)]
pub struct CustomerServiceLevelsRow {
    pub created_at: Option<NaiveDate>,
    pub min_service_level: Option<f64>,
    pub max_service_level: Option<f64>,
    pub average_service_level: Option<f64>,
    pub std_dev_service_level: Option<f64>,
}

/// Inline `customer_service_level_details` SQL — DATE cast, hocustcode, shortage, service_level.
#[derive(Serialize, FromRow)]
pub struct CustomerServiceLevelDetailRow {
    pub created_at: Option<NaiveDate>,
    pub hocustcode: Option<String>,
    pub shortage: Option<f64>,
    pub service_level: Option<f64>,
}

/// `supplier_service_levels.sql` — raw `created_at` (VARCHAR) plus min/max/avg/std_dev.
#[derive(Serialize, FromRow)]
pub struct SupplierServiceLevelsRow {
    pub created_at: Option<String>,
    pub min_service_level: Option<f64>,
    pub max_service_level: Option<f64>,
    pub average_service_level: Option<f64>,
    pub std_dev_service_level: Option<f64>,
}

/// `supplier_service_levels_details.sql` — raw `created_at`, supcode, service_level.
#[derive(Serialize, FromRow)]
pub struct SupplierServiceLevelDetailRow {
    pub created_at: Option<String>,
    pub supcode: Option<String>,
    pub service_level: Option<f64>,
}

/// `supplier_tier_service_levels.sql` — DATE cast, three pivoted tier columns.
#[derive(Serialize, FromRow)]
pub struct TierServiceLevelsRow {
    pub created_at: Option<NaiveDate>,
    pub premium_service_level: Option<f64>,
    pub standard_service_level: Option<f64>,
    pub value_service_level: Option<f64>,
}

/// `supplier_summary_metrics.sql`
#[derive(Serialize, FromRow)]
pub struct SupplierSummaryRow {
    pub created_at: Option<NaiveDate>,
    pub smallest_allocation: Option<f64>,
    pub number_of_skus: Option<i64>,
    pub number_of_customers: Option<i64>,
}

/// `customer_summary_metrics.sql`
#[derive(Serialize, FromRow)]
pub struct CustomerSummaryRow {
    pub created_at: Option<NaiveDate>,
    pub number_of_skus: Option<i64>,
    pub number_of_growers: Option<i64>,
}

/// `supply_and_demand.sql` — raw created_at plus three weight columns.
#[derive(Serialize, FromRow)]
pub struct SupplyDemandRow {
    pub created_at: Option<String>,
    pub supply_wgt: Option<f64>,
    pub demand_wgt: Option<f64>,
    pub allocated_wgt: Option<f64>,
}

/// `supply_and_demand_details.sql` — long-format rows: created_at, metric, value.
#[derive(Serialize, FromRow)]
pub struct SupplyDemandDetailRow {
    pub created_at: Option<String>,
    pub metric: Option<String>,
    pub value: Option<f64>,
}

/// `annual_deviation.sql` — per-ISO-week comparison of the most recent
/// weekly optimiser run vs the annual plan total.
#[derive(Serialize, FromRow)]
pub struct AnnualDeviationRow {
    pub iso_week: Option<i32>,
    pub weekly_wgt: Option<f64>,
    pub annual_wgt: Option<f64>,
}

// ── Handlers ─────────────────────────────────────────────────────────

async fn get_run_timestamps(State(state): State<AppState>) -> ApiResult<Json<Vec<RunTimestamp>>> {
    let rows: Vec<RunTimestamp> = state
        .db
        .load_data(
            r#"
            SELECT DISTINCT
                created_at::varchar AS run_timestamp,
                created_at::date::varchar AS run_date
            FROM {schema}.weekly_optimiser_output
            WHERE created_at IS NOT NULL
            ORDER BY run_timestamp DESC
            LIMIT 20
            "#,
            &Params::new(),
        )
        .await?;
    Ok(Json(rows))
}

async fn run_kpi_query<T>(state: &AppState, sql: &str, filters: &KpiFilters) -> ApiResult<Vec<T>>
where
    T: for<'r> FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    filters.validate()?;
    let params = filters.to_params();
    Ok(state.db.load_data(sql, &params).await?)
}

async fn get_cumulative_complexity(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<CumulativeComplexityRow>>> {
    Ok(Json(
        run_kpi_query(&state, CUMULATIVE_COMPLEXITY_SQL, &filters).await?,
    ))
}

async fn get_supplier_complexity(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<SupplierComplexityRow>>> {
    Ok(Json(
        run_kpi_query(&state, SUPPLIER_COMPLEXITY_SQL, &filters).await?,
    ))
}

async fn get_customer_complexity(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<CustomerComplexityRow>>> {
    Ok(Json(
        run_kpi_query(&state, CUSTOMER_COMPLEXITY_SQL, &filters).await?,
    ))
}

async fn get_customer_service_levels(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<CustomerServiceLevelsRow>>> {
    Ok(Json(
        run_kpi_query(&state, CUSTOMER_SERVICE_LEVELS_SQL, &filters).await?,
    ))
}

async fn get_customer_service_level_details(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<CustomerServiceLevelDetailRow>>> {
    Ok(Json(
        run_kpi_query(&state, CUSTOMER_SERVICE_LEVEL_DETAILS_SQL, &filters).await?,
    ))
}

async fn get_supplier_service_levels(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<SupplierServiceLevelsRow>>> {
    Ok(Json(
        run_kpi_query(&state, SUPPLIER_SERVICE_LEVELS_SQL, &filters).await?,
    ))
}

async fn get_supplier_service_level_details(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<SupplierServiceLevelDetailRow>>> {
    Ok(Json(
        run_kpi_query(&state, SUPPLIER_SERVICE_LEVELS_DETAILS_SQL, &filters).await?,
    ))
}

async fn get_supplier_tier_service_levels(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<TierServiceLevelsRow>>> {
    Ok(Json(
        run_kpi_query(&state, SUPPLIER_TIER_SERVICE_LEVELS_SQL, &filters).await?,
    ))
}

async fn get_supplier_summary_metrics(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<SupplierSummaryRow>>> {
    Ok(Json(
        run_kpi_query(&state, SUPPLIER_SUMMARY_METRICS_SQL, &filters).await?,
    ))
}

async fn get_customer_tier_service_levels(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<TierServiceLevelsRow>>> {
    Ok(Json(
        run_kpi_query(&state, CUSTOMER_TIER_SERVICE_LEVELS_SQL, &filters).await?,
    ))
}

async fn get_customer_summary_metrics(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<CustomerSummaryRow>>> {
    Ok(Json(
        run_kpi_query(&state, CUSTOMER_SUMMARY_METRICS_SQL, &filters).await?,
    ))
}

async fn get_supply_and_demand(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<SupplyDemandRow>>> {
    Ok(Json(
        run_kpi_query(&state, SUPPLY_AND_DEMAND_SQL, &filters).await?,
    ))
}

async fn get_supply_and_demand_details(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<SupplyDemandDetailRow>>> {
    Ok(Json(
        run_kpi_query(&state, SUPPLY_AND_DEMAND_DETAILS_SQL, &filters).await?,
    ))
}

async fn get_annual_deviation(
    State(state): State<AppState>,
    Query(filters): Query<KpiFilters>,
) -> ApiResult<Json<Vec<AnnualDeviationRow>>> {
    Ok(Json(
        run_kpi_query(&state, ANNUAL_DEVIATION_SQL, &filters).await?,
    ))
}
