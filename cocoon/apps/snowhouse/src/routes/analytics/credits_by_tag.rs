//! `GET /api/credits-by-tag` — estimated compute credits
//! grouped by `query_tag` (source / owner / env).

use axum::Json;
use axum::extract::{Query, State};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::response::WarehouseResponse;
use super::shared::{DaysParams, WAREHOUSE_CREDITS_PER_HOUR, history_source, validate_days};

/// Estimated compute credits grouped by `query_tag`, decomposed into the Nimbus
/// tag dimensions (source / owner / env). Untagged queries fall into an
/// '(untagged)' bucket so the tagged/untagged split stays visible. Same 7-day
/// ceiling.
pub fn query(days: u32) -> String {
    let source = history_source();
    format!(
        "WITH tagged AS (
            SELECT
                COALESCE(NULLIF(TRIM(query_tag), ''), '(untagged)') AS tag,
                TRY_PARSE_JSON(query_tag):\"nimbus@source\"::string AS tag_source,
                TRY_PARSE_JSON(query_tag):\"nimbus@owner\"::string AS tag_owner,
                TRY_PARSE_JSON(query_tag):\"nimbus@env\"::string AS tag_env,
                (execution_time / 1000.0 / 3600.0) * {WAREHOUSE_CREDITS_PER_HOUR} AS est_credits,
                total_elapsed_time
            FROM {source}
            WHERE start_time >= DATEADD(day, -{days}, CURRENT_TIMESTAMP())
        )
        SELECT
            tag AS query_tag,
            MAX(tag_source) AS tag_source,
            MAX(tag_owner) AS tag_owner,
            MAX(tag_env) AS tag_env,
            COUNT(*) AS query_count,
            ROUND(SUM(est_credits), 6) AS total_credits,
            ROUND(AVG(est_credits), 6) AS avg_credits,
            ROUND(SUM(total_elapsed_time) / 1000.0, 2) AS total_elapsed_seconds
        FROM tagged
        GROUP BY tag
        ORDER BY total_credits DESC"
    )
}

/// Get estimated credits grouped by query_tag (source / owner / env)
#[utoipa::path(get, path = "/api/credits-by-tag", tag = "Snowflake",
    params(DaysParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<DaysParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    let sql = query(params.days);
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_by_start_time_over_the_cache() {
        // Phase 2: the window is a plain WHERE filter over the owned cache table.
        // The table-function's END_TIME_RANGE args (and their 7-day quirk) are
        // gone, so the query no longer references them.
        let sql = query(7);
        assert!(sql.contains(&format!("FROM {}", history_source())));
        assert!(sql.contains("WHERE start_time >= DATEADD(day, -7, CURRENT_TIMESTAMP())"));
        assert!(!sql.contains("information_schema.query_history"));
        assert!(!sql.contains("END_TIME_RANGE_START"));
    }
}
