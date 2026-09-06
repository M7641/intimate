//! Aggregated overviews of the optimiser's latest output (radar KPIs, grower /
//! customer service levels, allocation pivot).

use std::collections::HashSet;

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use common_rs::db::{BindValue, Params};
use common_rs::validation::SAFE_CODE;

use crate::state::AppState;
use common_rs::error::{ApiError, ApiResult};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/output_overview/last_run_kpis", get(get_last_run_kpis))
        .route("/output_overview/grower_overview", get(get_grower_overview))
        .route(
            "/output_overview/customer_overview",
            get(get_customer_overview),
        )
        .route(
            "/output_overview/allocation_details",
            get(get_allocation_details),
        )
}

fn opt_text(v: &Option<String>) -> BindValue {
    match v {
        Some(s) => BindValue::Text(s.clone()),
        None => BindValue::Null,
    }
}

#[derive(Deserialize, Default)]
pub struct MascodeFilter {
    pub mascode: Option<String>,
}

impl MascodeFilter {
    fn validate(&self) -> Result<(), ApiError> {
        if let Some(v) = &self.mascode {
            if v.len() > 16 || !SAFE_CODE.is_match(v) {
                return Err(ApiError::BadRequest(format!("invalid mascode: {v:?}")));
            }
        }
        Ok(())
    }

    fn to_params(&self) -> Params {
        let mut params = Params::new();
        params.insert("mascode".to_string(), opt_text(&self.mascode));
        params
    }
}

#[derive(Deserialize, Default)]
pub struct AllocationFilters {
    pub mascode: Option<String>,
    pub supcode: Option<String>,
    pub hocustcode: Option<String>,
}

impl AllocationFilters {
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
        Ok(())
    }

    fn to_params(&self) -> Params {
        let mut params = Params::new();
        params.insert("mascode".to_string(), opt_text(&self.mascode));
        params.insert("supcode".to_string(), opt_text(&self.supcode));
        params.insert("hocustcode".to_string(), opt_text(&self.hocustcode));
        params
    }
}

#[derive(Serialize, FromRow)]
pub struct RadarKpiRow {
    pub mascode: Option<String>,
    pub pct_skus_zero_sl: f64,
    pub pct_growers_zero_sl: f64,
    pub max_unallocated_pct: f64,
    pub stddev_customer_sl: f64,
    pub stddev_supplier_sl: f64,
    pub pct_skus_one_grower: f64,
    pub pct_skus_mostly_one: f64,
    pub pct_small_allocations: f64,
    pub pct_growers_one_sku: f64,
    pub pct_growers_many_skus: f64,
}

#[derive(Serialize, FromRow)]
pub struct GrowerOverviewRow {
    pub supcode: Option<String>,
    pub mascode: Option<String>,
    pub overall_service_level: Option<f64>,
    pub premium_service_level: Option<f64>,
    pub standard_service_level: Option<f64>,
    pub value_service_level: Option<f64>,
    pub smallest_allocation: Option<f64>,
    pub number_of_skus: Option<i64>,
    pub number_of_customers: Option<i64>,
}

#[derive(Serialize, FromRow)]
pub struct CustomerOverviewRow {
    pub customer: Option<String>,
    pub mascode: Option<String>,
    pub overall_service_level: Option<f64>,
    pub premium_service_level: Option<f64>,
    pub standard_service_level: Option<f64>,
    pub value_service_level: Option<f64>,
    pub number_of_skus: Option<i64>,
    pub number_of_growers: Option<i64>,
}

#[derive(Serialize, FromRow, Clone)]
pub struct AllocationDetailRow {
    pub supcode: Option<String>,
    pub mascode: Option<String>,
    pub variety: Option<String>,
    pub hocustcode: Option<String>,
    pub tier: Option<String>,
    pub brand: Option<String>,
    pub countsize: Option<String>,
    pub product_code: Option<String>,
    pub weekly_estimate: Option<f64>,
    pub product_allocation: Option<f64>,
}

#[derive(Serialize)]
pub struct AllocationDetailsResponse {
    pub data: Vec<AllocationDetailRow>,
    pub product_codes: Vec<String>,
}

async fn get_last_run_kpis(
    State(state): State<AppState>,
    Query(filter): Query<MascodeFilter>,
) -> ApiResult<Json<Vec<RadarKpiRow>>> {
    filter.validate()?;
    let rows: Vec<RadarKpiRow> = state.db.load_data(RADAR_SQL, &filter.to_params()).await?;
    Ok(Json(rows))
}

async fn get_grower_overview(
    State(state): State<AppState>,
    Query(filter): Query<MascodeFilter>,
) -> ApiResult<Json<Vec<GrowerOverviewRow>>> {
    filter.validate()?;
    let rows: Vec<GrowerOverviewRow> = state
        .db
        .load_data(GROWER_OVERVIEW_SQL, &filter.to_params())
        .await?;
    Ok(Json(rows))
}

async fn get_customer_overview(
    State(state): State<AppState>,
    Query(filter): Query<MascodeFilter>,
) -> ApiResult<Json<Vec<CustomerOverviewRow>>> {
    filter.validate()?;
    let rows: Vec<CustomerOverviewRow> = state
        .db
        .load_data(CUSTOMER_OVERVIEW_SQL, &filter.to_params())
        .await?;
    Ok(Json(rows))
}

async fn get_allocation_details(
    State(state): State<AppState>,
    Query(filter): Query<AllocationFilters>,
) -> ApiResult<Json<AllocationDetailsResponse>> {
    filter.validate()?;
    let data: Vec<AllocationDetailRow> = state
        .db
        .load_data(ALLOCATION_DETAILS_SQL, &filter.to_params())
        .await?;

    let mut codes: Vec<String> = data
        .iter()
        .filter_map(|r| r.product_code.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    codes.sort();

    Ok(Json(AllocationDetailsResponse {
        data,
        product_codes: codes,
    }))
}

const RADAR_SQL: &str = r#"
WITH
    tiers AS (
        SELECT DISTINCT
            hocustcode,
            COALESCE(brand, '<NULL>') AS brand,
            COALESCE(tier_desc, 'STANDARD') AS tier
        FROM {schema}.weekly_demand_plan
    ),
    base_data AS (
        SELECT
            output.supcode,
            output.mascode,
            output.variety,
            output.countsize,
            output.supply_wgt,
            output.hocustcode,
            tiers.tier,
            output.demand_wgt,
            output.allocated_wgt
        FROM {schema}.weekly_optimiser_output output
        LEFT JOIN tiers USING(hocustcode, brand)
        WHERE %(mascode)s::varchar IS NULL
           OR LOWER(output.mascode) = LOWER(%(mascode)s)
    ),
    sku_service_levels AS (
        SELECT mascode, countsize,
            SUM(allocated_wgt) / NULLIF(SUM(demand_wgt), 0) AS sku_service_level
        FROM base_data GROUP BY mascode, countsize
    ),
    skus_zero_service AS (
        SELECT mascode,
            sum(case WHEN sku_service_level = 0 OR sku_service_level IS NULL THEN 1 END) zero_count,
            COUNT(*) AS total_count
        FROM sku_service_levels GROUP BY mascode
    ),
    grower_service_levels AS (
        SELECT mascode, supcode,
            SUM(allocated_wgt) / NULLIF(SUM(supply_wgt), 0) AS grower_service_level
        FROM base_data GROUP BY mascode, supcode
    ),
    growers_zero_service AS (
        SELECT mascode,
            sum(case WHEN grower_service_level = 0 OR grower_service_level IS NULL THEN 1 END) AS zero_count,
            COUNT(*) AS total_count
        FROM grower_service_levels GROUP BY mascode
    ),
    grower_unallocated AS (
        SELECT mascode, supcode,
            (SUM(supply_wgt) - SUM(allocated_wgt)) / NULLIF(SUM(supply_wgt), 0) AS unallocated_pct
        FROM base_data GROUP BY mascode, supcode
    ),
    max_grower_unallocated AS (
        SELECT mascode, MAX(unallocated_pct) AS max_unallocated
        FROM grower_unallocated GROUP BY mascode
    ),
    customer_service_levels AS (
        SELECT mascode, hocustcode,
            SUM(allocated_wgt) / NULLIF(SUM(demand_wgt), 0) AS customer_service_level
        FROM base_data GROUP BY mascode, hocustcode
    ),
    customer_stddev AS (
        SELECT mascode, STDDEV(customer_service_level) AS stddev_customer_sl
        FROM customer_service_levels GROUP BY mascode
    ),
    supplier_stddev AS (
        SELECT mascode, STDDEV(grower_service_level) AS stddev_supplier_sl
        FROM grower_service_levels GROUP BY mascode
    ),
    sku_grower_counts AS (
        SELECT mascode, countsize, COUNT(DISTINCT supcode) AS grower_count
        FROM base_data GROUP BY mascode, countsize
    ),
    skus_one_grower AS (
        SELECT mascode,
            sum(CASE WHEN grower_count = 1 THEN 1 END) AS one_grower_count,
            COUNT(*) AS total_count
        FROM sku_grower_counts GROUP BY mascode
    ),
    sku_allocations AS (
        SELECT mascode, countsize, supcode,
            SUM(allocated_wgt) AS grower_allocation,
            SUM(SUM(allocated_wgt)) OVER (PARTITION BY mascode, countsize) AS total_sku_allocation
        FROM base_data GROUP BY mascode, countsize, supcode
    ),
    sku_dominant_grower AS (
        SELECT mascode, countsize,
            MAX(grower_allocation / NULLIF(total_sku_allocation, 0)) AS max_grower_share
        FROM sku_allocations GROUP BY mascode, countsize
    ),
    skus_mostly_one AS (
        SELECT mascode,
            sum(CASE WHEN max_grower_share > 0.8 THEN 1 END) AS mostly_one_count,
            COUNT(*) AS total_count
        FROM sku_dominant_grower GROUP BY mascode
    ),
    small_allocations AS (
        SELECT mascode,
            sum(CASE WHEN allocated_wgt > 0 AND allocated_wgt < 100 THEN 1 END) small_count,
            sum(CASE WHEN allocated_wgt > 0 THEN 1 END) AS total_count
        FROM base_data GROUP BY mascode
    ),
    grower_sku_counts AS (
        SELECT mascode, supcode, COUNT(DISTINCT countsize) AS sku_count
        FROM base_data GROUP BY mascode, supcode
    ),
    growers_one_sku AS (
        SELECT mascode,
            sum(CASE WHEN sku_count = 1 THEN 1 END) AS one_sku_count,
            COUNT(*) AS total_count
        FROM grower_sku_counts GROUP BY mascode
    ),
    growers_many_skus AS (
        SELECT mascode,
            sum(CASE WHEN sku_count > 5 THEN 1 END) AS many_sku_count,
            COUNT(*) AS total_count
        FROM grower_sku_counts GROUP BY mascode
    )
SELECT
    COALESCE(skus_zero_service.mascode, growers_zero_service.mascode) AS mascode,
    COALESCE(skus_zero_service.zero_count::float / NULLIF(skus_zero_service.total_count, 0), 0) AS pct_skus_zero_sl,
    COALESCE(growers_zero_service.zero_count::float / NULLIF(growers_zero_service.total_count, 0), 0) AS pct_growers_zero_sl,
    COALESCE(max_grower_unallocated.max_unallocated, 0) AS max_unallocated_pct,
    COALESCE(customer_stddev.stddev_customer_sl, 0) AS stddev_customer_sl,
    COALESCE(supplier_stddev.stddev_supplier_sl, 0) AS stddev_supplier_sl,
    COALESCE(skus_one_grower.one_grower_count::float / NULLIF(skus_one_grower.total_count, 0), 0) AS pct_skus_one_grower,
    COALESCE(skus_mostly_one.mostly_one_count::float / NULLIF(skus_mostly_one.total_count, 0), 0) AS pct_skus_mostly_one,
    COALESCE(small_allocations.small_count::float / NULLIF(small_allocations.total_count, 0), 0) AS pct_small_allocations,
    COALESCE(growers_one_sku.one_sku_count::float / NULLIF(growers_one_sku.total_count, 0), 0) AS pct_growers_one_sku,
    COALESCE(growers_many_skus.many_sku_count::float / NULLIF(growers_many_skus.total_count, 0), 0) AS pct_growers_many_skus
FROM skus_zero_service
FULL OUTER JOIN growers_zero_service USING (mascode)
FULL OUTER JOIN max_grower_unallocated USING (mascode)
FULL OUTER JOIN customer_stddev USING (mascode)
FULL OUTER JOIN supplier_stddev USING (mascode)
FULL OUTER JOIN skus_one_grower USING (mascode)
FULL OUTER JOIN skus_mostly_one USING (mascode)
FULL OUTER JOIN small_allocations USING (mascode)
FULL OUTER JOIN growers_one_sku USING (mascode)
FULL OUTER JOIN growers_many_skus USING (mascode)
"#;

const GROWER_OVERVIEW_SQL: &str = r#"
WITH
    tiers AS (
        SELECT DISTINCT hocustcode,
            COALESCE(brand, '<NULL>') AS brand,
            COALESCE(tier_desc, 'STANDARD') AS tier
        FROM {schema}.weekly_demand_plan
    ),
    repair_tiers_data as (
        SELECT output.supcode, output.mascode, output.supply_wgt, output.hocustcode,
            tiers.tier, output.demand_wgt, output.allocated_wgt, output.countsize
        FROM {schema}.weekly_optimiser_output output
        LEFT JOIN tiers using(hocustcode, brand)
    ),
    grower_stats as (
        SELECT supcode, mascode,
            SUM(allocated_wgt) / NULLIF(SUM(supply_wgt), 0) as overall_service_level,
            SUM(CASE WHEN tier = 'PREMIUM' THEN allocated_wgt ELSE 0 END) /
                NULLIF(SUM(CASE WHEN tier = 'PREMIUM' THEN supply_wgt ELSE 0 END), 0) as premium_service_level,
            SUM(CASE WHEN tier = 'STANDARD' THEN allocated_wgt ELSE 0 END) /
                NULLIF(SUM(CASE WHEN tier = 'STANDARD' THEN supply_wgt ELSE 0 END), 0) as standard_service_level,
            SUM(CASE WHEN tier = 'VALUE' THEN allocated_wgt ELSE 0 END) /
                NULLIF(SUM(CASE WHEN tier = 'VALUE' THEN supply_wgt ELSE 0 END), 0) as value_service_level,
            MIN(allocated_wgt) as smallest_allocation,
            COUNT(DISTINCT countsize) as number_of_skus,
            COUNT(DISTINCT hocustcode) as number_of_customers
        FROM repair_tiers_data
        WHERE %(mascode)s::varchar IS NULL OR mascode = %(mascode)s
        GROUP BY supcode, mascode
    )
SELECT supcode, mascode, overall_service_level, premium_service_level,
    standard_service_level, value_service_level, smallest_allocation,
    number_of_skus, number_of_customers
FROM grower_stats
ORDER BY supcode, mascode
"#;

const CUSTOMER_OVERVIEW_SQL: &str = r#"
WITH
    tiers AS (
        SELECT DISTINCT hocustcode,
            COALESCE(brand, '<NULL>') AS brand,
            COALESCE(tier_desc, 'STANDARD') AS tier
        FROM {schema}.weekly_demand_plan
    ),
    repair_tiers_data as (
        SELECT output.supcode, output.mascode, output.hocustcode, tiers.tier,
            output.demand_wgt, output.allocated_wgt, output.countsize
        FROM {schema}.weekly_optimiser_output output
        LEFT JOIN tiers using(hocustcode, brand)
    ),
    customer_stats as (
        SELECT hocustcode, mascode,
            SUM(allocated_wgt) / NULLIF(SUM(demand_wgt), 0) as overall_service_level,
            SUM(CASE WHEN tier = 'PREMIUM' THEN allocated_wgt ELSE 0 END) /
                NULLIF(SUM(CASE WHEN tier = 'PREMIUM' THEN demand_wgt ELSE 0 END), 0) as premium_service_level,
            SUM(CASE WHEN tier = 'STANDARD' THEN allocated_wgt ELSE 0 END) /
                NULLIF(SUM(CASE WHEN tier = 'STANDARD' THEN demand_wgt ELSE 0 END), 0) as standard_service_level,
            SUM(CASE WHEN tier = 'VALUE' THEN allocated_wgt ELSE 0 END) /
                NULLIF(SUM(CASE WHEN tier = 'VALUE' THEN demand_wgt ELSE 0 END), 0) as value_service_level,
            COUNT(DISTINCT countsize) as number_of_skus,
            COUNT(DISTINCT supcode) as number_of_growers
        FROM repair_tiers_data
        WHERE %(mascode)s::varchar IS NULL OR LOWER(mascode) = LOWER(%(mascode)s)
        GROUP BY hocustcode, mascode
    )
SELECT hocustcode::text as customer, mascode, overall_service_level,
    premium_service_level, standard_service_level, value_service_level,
    number_of_skus, number_of_growers
FROM customer_stats
ORDER BY hocustcode, mascode
"#;

const ALLOCATION_DETAILS_SQL: &str = r#"
WITH allocation_data AS (
    SELECT supcode::varchar as supcode, mascode, variety, hocustcode, tier, brand, countsize,
        MAX(supply_wgt) as weekly_estimate,
        SUM(allocated_wgt) as product_allocation
    FROM {schema}.weekly_optimiser_output output
    WHERE 1=1
    AND (%(mascode)s::varchar IS NULL OR mascode = %(mascode)s)
    AND (%(supcode)s::varchar IS NULL OR supcode::varchar = %(supcode)s)
    AND (%(hocustcode)s::varchar IS NULL OR hocustcode::varchar = %(hocustcode)s)
    GROUP BY supcode, mascode, variety, hocustcode, tier, brand, countsize
)
SELECT supcode, mascode, variety, hocustcode, tier, brand, countsize,
    (hocustcode || ' ' || tier || ' ' || brand || ' ' || countsize || ' ' || mascode) AS product_code,
    weekly_estimate, product_allocation
FROM allocation_data
WHERE (hocustcode || ' ' || tier || ' ' || brand || ' ' || countsize || ' ' || mascode) IS NOT NULL
ORDER BY supcode, mascode, variety
"#;
