//! `GET /api/cost-by-table` — cost grouped by the table
//! each write statement targets.

use axum::Json;
use axum::extract::{Query, State};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::response::WarehouseResponse;
use super::shared::{
    ExpensiveParams, WAREHOUSE_CREDITS_PER_HOUR, history_source, validate_days, validate_limit,
};

/// Cost grouped by the table each write statement targets.
///
/// The target table is extracted from `query_text` with a regex (the object
/// after CREATE/MERGE/INSERT/COPY/UPDATE/DELETE/TRUNCATE), so read-only queries
/// (no target) drop out. Credits are estimated the same way as the expensive
/// queries view (execution time × warehouse-size multiplier) and reported in
/// estimated credits only. Optional `search` filters on the table name.
pub fn query(days: u32, top_n: u32, search: Option<&str>) -> String {
    let search_clause = match search {
        Some(s) if !s.trim().is_empty() => {
            let escaped = s.replace('\'', "''");
            format!("AND target_table ILIKE '%{escaped}%'")
        }
        _ => String::new(),
    };
    // Extract the identifier after a write keyword. Snowflake's regex engine has
    // NO non-capturing `(?:...)` groups, so every group here captures; the table
    // identifier is therefore capture group 6 (the wrapping keyword group is 1,
    // the four optional keyword sub-groups are 2-5). Dollar-quoted in SQL
    // ($$...$$) so backslashes reach the regex engine untouched.
    let pattern = "(create\\s+(or\\s+replace\\s+)?(transient\\s+|temporary\\s+|temp\\s+|volatile\\s+)?table\\s+(if\\s+not\\s+exists\\s+)?|merge\\s+into\\s+|insert\\s+(overwrite\\s+)?into\\s+|copy\\s+into\\s+|update\\s+|delete\\s+from\\s+|truncate\\s+table\\s+)([a-zA-Z0-9_.$\"]+)";
    let source = history_source();
    format!(
        "WITH tagged AS (
            SELECT
                LOWER(REPLACE(REGEXP_SUBSTR(query_text, $${pattern}$$, 1, 1, 'ie', 6), '\"', '')) AS target_table,
                bytes_scanned,
                rows_produced,
                total_elapsed_time,
                (execution_time / 1000.0 / 3600.0) * {WAREHOUSE_CREDITS_PER_HOUR} AS est_credits
            FROM {source}
            WHERE start_time >= DATEADD(day, -{days}, CURRENT_TIMESTAMP())
        )
        SELECT
            target_table,
            COUNT(*) as write_count,
            SUM(rows_produced) as total_rows_created,
            ROUND(AVG(rows_produced), 0) as avg_rows_created,
            ROUND(SUM(bytes_scanned) / (1024 * 1024 * 1024), 4) as total_gb_scanned,
            ROUND(SUM(total_elapsed_time) / 1000, 2) as total_elapsed_seconds,
            ROUND(SUM(est_credits), 6) as total_credits
        FROM tagged
        WHERE target_table IS NOT NULL AND target_table <> ''
            {search_clause}
        GROUP BY target_table
        ORDER BY total_credits DESC
        LIMIT {top_n}"
    )
}

/// Get cost grouped by the table each write statement targets
#[utoipa::path(get, path = "/api/cost-by-table", tag = "Snowflake",
    params(ExpensiveParams),
    responses((status = 200, body = WarehouseResponse), (status = 500, body = ErrorResponse)))]
#[tracing::instrument(skip_all)]
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<ExpensiveParams>,
) -> Result<Json<WarehouseResponse>, AppError> {
    validate_days(params.days)?;
    validate_limit(params.top_n)?;
    let sql = query(params.days, params.top_n, params.search.as_deref());
    let rows = state.blocking_query(sql).await?;
    Ok(Json(WarehouseResponse::from_rows(rows)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_is_snowflake_safe() {
        let sql = query(7, 50, None);
        // Snowflake's regex engine rejects non-capturing `(?:...)` groups — this
        // is the bug that broke the endpoint at runtime. Every group must capture.
        assert!(!sql.contains("(?:"));
        // With all groups capturing, the table identifier is group 6.
        assert!(sql.contains("'ie', 6)"));
    }
}
