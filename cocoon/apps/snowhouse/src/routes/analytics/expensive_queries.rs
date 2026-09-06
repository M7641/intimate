//! `GET /api/expensive-queries` — the most expensive
//! queries by estimated compute cost.

use axum::Json;
use axum::extract::{Query, State};

use service_kit::error::{AppError, ErrorResponse};
use service_kit::state::AppState;

use super::response::WarehouseResponse;
use super::shared::{
    ExpensiveParams, WAREHOUSE_CREDITS_PER_HOUR, history_source, validate_days, validate_limit,
};

/// Most expensive queries by estimated compute cost.
///
/// Reads the owned query-history cache (Phase 2 — see decision 0004), so it is
/// not bound by the `information_schema` table function's 10k-row cap or 7-day
/// retention: `days` is limited only by how much history the cache holds.
///
/// Cost is reported in estimated credits only (no monetary conversion).
pub fn query(days: u32, top_n: u32, search: Option<&str>) -> String {
    // Optional case-insensitive substring search over the SQL text. Single quotes
    // are doubled so the needle stays safely inside the string literal.
    let search_clause = match search {
        Some(s) if !s.trim().is_empty() => {
            let escaped = s.replace('\'', "''");
            format!("AND query_text ILIKE '%{escaped}%'")
        }
        _ => String::new(),
    };
    let source = history_source();
    format!(
        "WITH scored AS (
            SELECT
                query_id,
                query_type,
                user_name,
                warehouse_name,
                warehouse_size,
                ROUND(execution_time / 1000, 2) as execution_seconds,
                ROUND(total_elapsed_time / 1000, 2) as total_elapsed_seconds,
                -- Time the query spent queued before running. Snowflake splits this
                -- into overload (warehouse was saturated — the contention signal)
                -- and provisioning (warehouse was cold-starting). Either can be NULL,
                -- so COALESCE each term before summing.
                ROUND((COALESCE(queued_overload_time, 0) + COALESCE(queued_provisioning_time, 0)) / 1000, 2) as queued_seconds,
                ROUND(COALESCE(queued_overload_time, 0) / 1000, 2) as queued_overload_seconds,
                ROUND(COALESCE(queued_provisioning_time, 0) / 1000, 2) as queued_provisioning_seconds,
                ROUND(bytes_scanned / (1024 * 1024 * 1024), 4) as gb_scanned,
                rows_produced,
                ROUND(credits_used_cloud_services, 6) as cloud_services_credits,
                ROUND(
                    (execution_time / 1000.0 / 3600.0) * {WAREHOUSE_CREDITS_PER_HOUR},
                6) as estimated_credits,
                TO_CHAR(start_time, 'YYYY-MM-DD\"T\"HH24:MI:SSTZH:TZM') as start_time,
                SUBSTRING(query_text, 1, 200) as query_preview,
                query_text
            FROM {source}
            WHERE start_time >= DATEADD(day, -{days}, CURRENT_TIMESTAMP())
                {search_clause}
        )
        SELECT scored.*
        FROM scored
        ORDER BY estimated_credits DESC
        LIMIT {top_n}"
    )
}

/// Get most expensive queries by cost
#[utoipa::path(get, path = "/api/expensive-queries", tag = "Snowflake",
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
    fn reports_credits_and_reads_the_cache() {
        let sql = query(7, 50, None);
        // Cost is estimated credits only — no monetary conversion.
        assert!(sql.contains("as estimated_credits"));
        assert!(!sql.to_lowercase().contains("cost_usd"));
        // Phase 2: reads the owned cache table, not the capped table function or
        // the admin-only ACCOUNT_USAGE view.
        assert!(sql.contains(&format!("FROM {}", history_source())));
        assert!(!sql.contains("information_schema.query_history"));
        assert!(!sql.to_lowercase().contains("account_usage"));
    }

    #[test]
    fn search_is_quote_safe() {
        let sql = query(7, 50, Some("o'brien"));
        // Single quotes are doubled so the needle stays inside the literal.
        assert!(sql.contains("ILIKE '%o''brien%'"));
    }
}
