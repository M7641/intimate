/// CTE to map stl_scan.tbl -> schema_name / table_name via pg_class.
const TABLE_ID_MAPPING_CTE: &str = "
    table_id_mappings AS (
        SELECT
            n.nspname AS schema_name,
            c.relname AS table_name,
            c.oid AS table_id
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relkind = 'r'
            AND n.nspname NOT IN ('pg_catalog', 'information_schema')
    )";

pub fn table_scans_query(days: u32, schema: Option<&str>) -> String {
    let schema_clause = schema
        .map(|s| format!("AND tim.schema_name = '{s}'"))
        .unwrap_or_default();

    format!(
        "WITH {TABLE_ID_MAPPING_CTE}
        SELECT
            tim.schema_name,
            tim.table_name,
            COUNT(*) as scan_count,
            COUNT(DISTINCT s.query) as unique_queries,
            COUNT(DISTINCT s.userid) as unique_users,
            SUM(s.rows) as total_rows_scanned,
            AVG(s.rows) as avg_rows_per_scan,
            MAX(s.rows) as max_rows_scanned,
            SUM(s.rows_pre_filter) as total_rows_pre_filter,
            CASE
                WHEN SUM(s.rows_pre_filter) > 0
                THEN ROUND(100.0 * (SUM(s.rows_pre_filter) - SUM(s.rows)) / SUM(s.rows_pre_filter), 2)
                ELSE 0
            END as filter_efficiency_pct,
            MIN(s.starttime) as first_scan,
            MAX(s.endtime) as last_scan,
            ROUND(SUM(s.bytes) / (1024.0 * 1024.0 * 1024.0), 2) as total_gb_scanned,
            COUNT(DISTINCT s.slice) as slices_used,
            COUNT(DISTINCT DATE(s.starttime)) as days_with_activity
        FROM stl_scan s
        JOIN table_id_mappings tim ON tim.table_id = s.tbl
        WHERE s.starttime >= DATEADD(day, -{days}, GETDATE())
            {schema_clause}
        GROUP BY 1, 2
        ORDER BY scan_count DESC"
    )
}

pub fn query_performance_query(days: u32) -> String {
    format!(
        "WITH {TABLE_ID_MAPPING_CTE}
        SELECT
            tim.schema_name,
            tim.table_name,
            COUNT(DISTINCT s.query) as query_count,
            ROUND(AVG(qms.query_cpu_time) / 1000000.0, 2) as avg_cpu_sec,
            ROUND(MAX(qms.query_cpu_time) / 1000000.0, 2) as max_cpu_sec,
            ROUND(AVG(qms.query_execution_time) / 1000.0, 2) as avg_exec_sec,
            ROUND(MAX(qms.query_execution_time) / 1000.0, 2) as max_exec_sec,
            ROUND(AVG(qms.query_blocks_read), 0) as avg_blocks_read,
            ROUND(SUM(qms.query_blocks_read), 0) as total_blocks_read,
            ROUND(AVG(qms.query_cpu_usage_percent), 2) as avg_cpu_usage_pct,
            ROUND(AVG(qms.query_temp_blocks_to_disk), 0) as avg_temp_blocks_to_disk,
            COUNT(CASE WHEN qms.query_temp_blocks_to_disk > 0 THEN 1 END) as queries_spilled_to_disk
        FROM stl_scan s
        JOIN table_id_mappings tim ON tim.table_id = s.tbl
        JOIN svl_query_metrics_summary qms ON s.query = qms.query
        WHERE s.starttime >= DATEADD(day, -{days}, GETDATE())
        GROUP BY 1, 2
        ORDER BY total_blocks_read DESC"
    )
}

pub fn compression_query(schema: Option<&str>) -> String {
    let schema_clause = match schema {
        Some(s) => format!("WHERE schemaname = '{s}'"),
        None => "WHERE schemaname NOT IN ('pg_catalog', 'information_schema', 'pg_internal')"
            .to_string(),
    };

    format!(
        r#"SELECT
            schemaname as schema_name,
            tablename as table_name,
            COUNT(*) as total_columns,
            SUM(CASE WHEN encoding = 'none' THEN 1 ELSE 0 END) as uncompressed_cols,
            SUM(CASE WHEN encoding = 'az64' THEN 1 ELSE 0 END) as az64_cols,
            SUM(CASE WHEN encoding = 'zstd' THEN 1 ELSE 0 END) as zstd_cols,
            SUM(CASE WHEN encoding = 'lzo' THEN 1 ELSE 0 END) as lzo_cols,
            SUM(CASE WHEN encoding = 'bytedict' THEN 1 ELSE 0 END) as bytedict_cols,
            SUM(CASE WHEN encoding LIKE 'delta%' THEN 1 ELSE 0 END) as delta_cols,
            SUM(CASE WHEN encoding = 'runlength' THEN 1 ELSE 0 END) as runlength_cols,
            SUM(CASE WHEN encoding = 'raw' THEN 1 ELSE 0 END) as raw_cols,
            ROUND(100.0 * SUM(CASE WHEN encoding = 'none' THEN 1 ELSE 0 END) / COUNT(*), 1) as uncompressed_pct,
            MAX(CASE WHEN sortkey > 0 THEN "column" ELSE NULL END) as sort_key,
            MAX(CASE WHEN distkey = true THEN "column" ELSE NULL END) as dist_key
        FROM pg_table_def
        {schema_clause}
        GROUP BY 1, 2
        ORDER BY uncompressed_cols DESC, total_columns DESC"#
    )
}

pub fn access_patterns_query(days: u32) -> String {
    format!(
        "WITH {TABLE_ID_MAPPING_CTE}
        SELECT
            tim.schema_name,
            tim.table_name,
            COUNT(*) as total_accesses,
            COUNT(DISTINCT DATE(s.starttime)) as days_accessed,
            COUNT(DISTINCT EXTRACT(hour FROM s.starttime)) as hours_accessed,
            MIN(s.starttime) as first_access,
            MAX(s.endtime) as last_access,
            DATEDIFF(day, MAX(s.endtime), GETDATE()) as days_since_last_access,
            SUM(CASE WHEN EXTRACT(dow FROM s.starttime) = 0 THEN 1 ELSE 0 END) as sunday_accesses,
            SUM(CASE WHEN EXTRACT(dow FROM s.starttime) = 1 THEN 1 ELSE 0 END) as monday_accesses,
            SUM(CASE WHEN EXTRACT(dow FROM s.starttime) = 2 THEN 1 ELSE 0 END) as tuesday_accesses,
            SUM(CASE WHEN EXTRACT(dow FROM s.starttime) = 3 THEN 1 ELSE 0 END) as wednesday_accesses,
            SUM(CASE WHEN EXTRACT(dow FROM s.starttime) = 4 THEN 1 ELSE 0 END) as thursday_accesses,
            SUM(CASE WHEN EXTRACT(dow FROM s.starttime) = 5 THEN 1 ELSE 0 END) as friday_accesses,
            SUM(CASE WHEN EXTRACT(dow FROM s.starttime) = 6 THEN 1 ELSE 0 END) as saturday_accesses,
            CASE
                WHEN COUNT(DISTINCT DATE(s.starttime)) >= 25 THEN 'Daily'
                WHEN COUNT(DISTINCT DATE(s.starttime)) >= 15 THEN 'Regular'
                WHEN COUNT(DISTINCT DATE(s.starttime)) >= 5 THEN 'Occasional'
                WHEN COUNT(DISTINCT DATE(s.starttime)) >= 1 THEN 'Rare'
                ELSE 'None'
            END as usage_pattern
        FROM stl_scan s
        JOIN table_id_mappings tim ON tim.table_id = s.tbl
        WHERE s.starttime >= DATEADD(day, -{days}, GETDATE())
        GROUP BY 1, 2
        ORDER BY total_accesses DESC"
    )
}

pub fn slow_queries_query(min_seconds: u32, days: u32) -> String {
    let min_ms = min_seconds * 1000;
    format!(
        "WITH {TABLE_ID_MAPPING_CTE}
        SELECT
            s.query,
            tim.schema_name,
            tim.table_name,
            s.userid,
            TRIM(u.usename) as username,
            s.starttime,
            ROUND(qms.query_execution_time / 1000.0, 2) as execution_time_sec,
            ROUND(qms.query_cpu_time / 1000000.0, 2) as cpu_time_sec,
            qms.query_blocks_read,
            qms.query_temp_blocks_to_disk,
            s.rows as rows_scanned,
            s.rows_pre_filter,
            CASE
                WHEN s.rows_pre_filter > 0
                THEN ROUND(100.0 * (s.rows_pre_filter - s.rows) / s.rows_pre_filter, 2)
                ELSE 0
            END as filter_efficiency_pct,
            SUBSTRING(q.querytxt, 1, 100) as query_preview
        FROM stl_scan s
        JOIN table_id_mappings tim ON tim.table_id = s.tbl
        JOIN svl_query_metrics_summary qms ON s.query = qms.query
        LEFT JOIN pg_user u ON s.userid = u.usesysid
        LEFT JOIN stl_query q ON s.query = q.query
        WHERE s.starttime >= DATEADD(day, -{days}, GETDATE())
            AND qms.query_execution_time >= {min_ms}
        ORDER BY qms.query_execution_time DESC
        LIMIT 100"
    )
}

/// Returns the two SQL queries needed for unused-tables analysis:
/// a scan-history query and an all-tables query.
///
/// Each query is self-contained — the table query sets its own
/// `search_path` via `set_config()` in a CTE so `pg_table_def`
/// sees all configured schemas.
pub fn unused_tables_queries(days_threshold: u32) -> (String, String) {
    let lookback = days_threshold * 2;

    let scan_query = format!(
        "WITH {TABLE_ID_MAPPING_CTE}
        SELECT DISTINCT
            lower(trim(tim.schema_name)) as schema_name,
            lower(trim(tim.table_name)) as table_name,
            TO_CHAR(MAX(s.endtime), 'YYYY-MM-DD HH24:MI:SS') as last_accessed
        FROM stl_scan s
        JOIN table_id_mappings tim ON tim.table_id = s.tbl
        WHERE s.starttime >= DATEADD(day, -{lookback}, GETDATE())
        GROUP BY 1, 2"
    );

    let table_query = "WITH _sp AS (
            SELECT set_config('search_path', '$user,public,stage,publish,transform,sandpit', false)
        )
        SELECT
            lower(trim(schemaname)) as schema_name,
            lower(trim(tablename)) as table_name,
            COUNT(DISTINCT \"column\") as column_count,
            MAX(sortkey) as has_sort_key,
            MAX(CASE WHEN distkey THEN 1 ELSE 0 END) as has_dist_key
        FROM _sp, pg_table_def
        WHERE schemaname NOT IN ('pg_catalog', 'information_schema', 'pg_internal')
        GROUP BY 1, 2"
        .to_string();

    (scan_query, table_query)
}

pub fn disk_queries_query(days: u32) -> String {
    format!(
        "WITH {TABLE_ID_MAPPING_CTE}
        SELECT
            s.query,
            tim.schema_name,
            tim.table_name,
            s.starttime,
            ROUND(qms.query_execution_time / 1000.0, 2) as execution_time_sec,
            qms.query_temp_blocks_to_disk,
            ROUND(qms.query_temp_blocks_to_disk * 1.0 / 1024, 2) as spilled_mb,
            s.rows as rows_scanned,
            qms.query_blocks_read,
            SUBSTRING(q.querytxt, 1, 150) as query_preview
        FROM stl_scan s
        JOIN table_id_mappings tim ON tim.table_id = s.tbl
        JOIN svl_query_metrics_summary qms ON s.query = qms.query
        LEFT JOIN stl_query q ON s.query = q.query
        WHERE s.starttime >= DATEADD(day, -{days}, GETDATE())
            AND qms.query_temp_blocks_to_disk > 0
        ORDER BY qms.query_temp_blocks_to_disk DESC
        LIMIT 50"
    )
}

pub fn table_sizes_query(schema: Option<&str>) -> String {
    let schema_clause = schema
        .map(|s| format!("AND tim.schema_name = '{s}'"))
        .unwrap_or_default();

    format!(
        "WITH {TABLE_ID_MAPPING_CTE}
        SELECT
            tim.schema_name,
            tim.table_name,
            MAX(s.rows) as estimated_rows,
            MAX(s.rows_pre_filter) as estimated_rows_pre_filter,
            ROUND(MAX(s.bytes) / (1024.0 * 1024.0), 2) as estimated_mb,
            ROUND(MAX(s.bytes) / (1024.0 * 1024.0 * 1024.0), 2) as estimated_gb,
            MAX(s.starttime) as last_full_scan
        FROM stl_scan s
        JOIN table_id_mappings tim ON tim.table_id = s.tbl
        WHERE s.starttime >= DATEADD(day, -7, GETDATE())
            {schema_clause}
        GROUP BY 1, 2
        ORDER BY estimated_mb DESC"
    )
}

/// Real on-disk storage per table, from `svv_table_info` — the canonical source
/// for a storage breakdown (every table, actual block count), unlike the
/// scan-derived `table_sizes_query`. `size` is in 1 MB blocks; `unsorted` and
/// `pct_used` surface reclaimable space. Visible rows depend on privileges
/// (superuser sees all; otherwise owned tables).
pub fn storage_query(schema: Option<&str>) -> String {
    let schema_clause = schema
        .map(|s| format!("AND \"schema\" = '{s}'"))
        .unwrap_or_default();

    format!(
        "SELECT
            \"schema\" AS schema_name,
            \"table\" AS table_name,
            size AS size_mb,
            ROUND(size / 1024.0, 3) AS size_gb,
            tbl_rows AS estimated_rows,
            pct_used,
            unsorted AS pct_unsorted,
            diststyle,
            encoded
        FROM svv_table_info
        WHERE \"schema\" NOT IN ('pg_catalog', 'information_schema')
            {schema_clause}
        ORDER BY size DESC"
    )
}

pub fn user_activity_query(days: u32) -> String {
    format!(
        "WITH {TABLE_ID_MAPPING_CTE}
        SELECT
            TRIM(u.usename) as username,
            tim.schema_name,
            tim.table_name,
            COUNT(*) as access_count,
            COUNT(DISTINCT s.query) as unique_queries,
            MIN(s.starttime) as first_access,
            MAX(s.endtime) as last_access
        FROM stl_scan s
        JOIN table_id_mappings tim ON tim.table_id = s.tbl
        LEFT JOIN pg_user u ON s.userid = u.usesysid
        WHERE s.starttime >= DATEADD(day, -{days}, GETDATE())
        GROUP BY 1, 2, 3
        ORDER BY access_count DESC"
    )
}

pub fn distribution_query() -> String {
    "SELECT
            n.nspname as schema_name,
            c.relname as table_name,
            MAX(CASE WHEN a.attisdistkey = true THEN a.attname ELSE NULL END) as dist_key_column,
            MAX(CASE
                WHEN pci.reldiststyle = 0 THEN 'EVEN'
                WHEN pci.reldiststyle = 1 THEN 'KEY'
                WHEN pci.reldiststyle = 8 THEN 'ALL'
                WHEN pci.reldiststyle = 10 THEN 'AUTO(ALL)'
                WHEN pci.reldiststyle = 11 THEN 'AUTO(EVEN)'
                ELSE 'AUTO/UNKNOWN'
            END) as dist_style,
            COUNT(DISTINCT a.attname) as total_columns,
            MAX(CASE WHEN a.attsortkeyord > 0 THEN a.attname ELSE NULL END) as sort_key_column,
            MAX(a.attsortkeyord) as sort_key_position,
            CASE
                WHEN MAX(CASE
                    WHEN pci.reldiststyle = 0 THEN 'EVEN'
                    WHEN pci.reldiststyle = 1 THEN 'KEY'
                    WHEN pci.reldiststyle = 8 THEN 'ALL'
                    ELSE 'OTHER'
                END) = 'EVEN' THEN 'Even distribution - good for small tables'
                WHEN MAX(CASE
                    WHEN pci.reldiststyle = 0 THEN 'EVEN'
                    WHEN pci.reldiststyle = 1 THEN 'KEY'
                    WHEN pci.reldiststyle = 8 THEN 'ALL'
                    ELSE 'OTHER'
                END) = 'KEY' AND MAX(CASE WHEN a.attisdistkey = true THEN a.attname END) IS NOT NULL
                    THEN 'Key distribution - check for skew'
                WHEN MAX(CASE
                    WHEN pci.reldiststyle = 0 THEN 'EVEN'
                    WHEN pci.reldiststyle = 1 THEN 'KEY'
                    WHEN pci.reldiststyle = 8 THEN 'ALL'
                    ELSE 'OTHER'
                END) = 'ALL' THEN 'All distribution - good for small lookup tables'
                ELSE 'Auto/Unknown'
            END as distribution_notes
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_class_info pci ON pci.reloid = c.oid
        LEFT JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0
        WHERE c.relkind = 'r'
            AND n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_internal')
        GROUP BY 1, 2
        ORDER BY 1, 2"
        .to_string()
}

pub fn filter_effectiveness_query(days: u32, min_scans: u32) -> String {
    format!(
        "WITH {TABLE_ID_MAPPING_CTE}
        SELECT
            tim.schema_name,
            tim.table_name,
            COUNT(*) as scan_count,
            SUM(s.rows) as total_rows_returned,
            SUM(s.rows_pre_filter) as total_rows_scanned,
            ROUND(AVG(CASE
                WHEN s.rows_pre_filter > 0
                THEN 100.0 * (s.rows_pre_filter - s.rows) / s.rows_pre_filter
                ELSE 0
            END), 2) as avg_filter_efficiency_pct,
            ROUND(MIN(CASE
                WHEN s.rows_pre_filter > 0
                THEN 100.0 * (s.rows_pre_filter - s.rows) / s.rows_pre_filter
                ELSE 0
            END), 2) as min_filter_efficiency_pct,
            ROUND(MAX(CASE
                WHEN s.rows_pre_filter > 0
                THEN 100.0 * (s.rows_pre_filter - s.rows) / s.rows_pre_filter
                ELSE 0
            END), 2) as max_filter_efficiency_pct,
            COUNT(CASE WHEN s.rows = s.rows_pre_filter THEN 1 END) as full_table_scans,
            ROUND(100.0 * COUNT(CASE WHEN s.rows = s.rows_pre_filter THEN 1 END) / COUNT(*), 1) as full_scan_pct,
            CASE
                WHEN AVG(CASE WHEN s.rows_pre_filter > 0
                    THEN 100.0 * (s.rows_pre_filter - s.rows) / s.rows_pre_filter ELSE 0 END) < 10
                    THEN 'Poor - Consider adding filters or indexes'
                WHEN AVG(CASE WHEN s.rows_pre_filter > 0
                    THEN 100.0 * (s.rows_pre_filter - s.rows) / s.rows_pre_filter ELSE 0 END) < 50
                    THEN 'Moderate - Some filtering happening'
                WHEN AVG(CASE WHEN s.rows_pre_filter > 0
                    THEN 100.0 * (s.rows_pre_filter - s.rows) / s.rows_pre_filter ELSE 0 END) < 90
                    THEN 'Good - Effective filtering'
                ELSE 'Excellent - Very selective queries'
            END as filter_assessment
        FROM stl_scan s
        JOIN table_id_mappings tim ON tim.table_id = s.tbl
        WHERE s.starttime >= DATEADD(day, -{days}, GETDATE())
            AND s.rows_pre_filter > 0
        GROUP BY 1, 2
        HAVING COUNT(*) >= {min_scans}
        ORDER BY avg_filter_efficiency_pct ASC, scan_count DESC"
    )
}

pub fn query_frequency_query(days: u32) -> String {
    format!(
        "WITH {TABLE_ID_MAPPING_CTE}
        SELECT
            EXTRACT(hour FROM s.starttime) as hour_of_day,
            COUNT(*) as total_scans,
            COUNT(DISTINCT s.query) as unique_queries,
            COUNT(DISTINCT tim.schema_name || '.' || tim.table_name) as unique_tables,
            ROUND(AVG(s.rows), 0) as avg_rows_scanned,
            ROUND(SUM(s.bytes) / (1024.0 * 1024.0 * 1024.0), 2) as total_gb_scanned
        FROM stl_scan s
        JOIN table_id_mappings tim ON tim.table_id = s.tbl
        WHERE s.starttime >= DATEADD(day, -{days}, GETDATE())
        GROUP BY 1
        ORDER BY 1"
    )
}

pub fn recent_queries_query(days: u32, limit: u32) -> String {
    format!(
        "SELECT
            q.query as query_id,
            TRIM(q.querytxt) as query_text,
            TRIM(u.usename) as user_name,
            q.starttime as start_time,
            q.endtime as end_time,
            ROUND(DATEDIFF(millisecond, q.starttime, q.endtime) / 1000.0, 2) as execution_seconds,
            q.aborted,
            q.label
        FROM stl_query q
        LEFT JOIN pg_user u ON q.userid = u.usesysid
        WHERE q.starttime >= DATEADD(day, -{days}, GETDATE())
            AND q.userid > 1
            AND TRIM(q.querytxt) NOT LIKE 'padb_fetch%%'
            AND TRIM(q.querytxt) NOT LIKE 'Undoing%%'
        ORDER BY q.starttime DESC
        LIMIT {limit}"
    )
}

pub fn query_plan_query(query_id: i64) -> String {
    format!(
        "SELECT
            nodeid as node_id,
            parentid as parent_id,
            TRIM(plannode) as plan_node,
            TRIM(info) as info
        FROM stl_explain
        WHERE query = {query_id}
        ORDER BY nodeid"
    )
}
