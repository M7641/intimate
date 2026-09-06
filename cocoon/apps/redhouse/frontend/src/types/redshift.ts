export interface RedshiftInfo {
  database: string;
}

export interface ApiResponse<T> {
  data: T[];
  columns: string[];
}

/// One row of real on-disk storage, from `svv_table_info`.
export interface StorageTable {
  schema_name: string;
  table_name: string;
  size_mb: number;
  size_gb: number;
  estimated_rows: number;
  pct_used: number;
  pct_unsorted: number;
  diststyle: string;
  encoded: string;
}

export interface TableScan {
  schema_name: string;
  table_name: string;
  scan_count: number;
  unique_queries: number;
  unique_users: number;
  total_rows_scanned: number;
  avg_rows_per_scan: number;
  max_rows_scanned: number;
  total_rows_pre_filter: number;
  filter_efficiency_pct: number;
  first_scan: string;
  last_scan: string;
  total_gb_scanned: number;
  slices_used: number;
  days_with_activity: number;
}

export interface QueryPerformance {
  schema_name: string;
  table_name: string;
  query_count: number;
  avg_cpu_sec: number;
  max_cpu_sec: number;
  avg_exec_sec: number;
  max_exec_sec: number;
  avg_blocks_read: number;
  total_blocks_read: number;
  avg_cpu_usage_pct: number;
  avg_temp_blocks_to_disk: number;
  queries_spilled_to_disk: number;
}

export interface TableCompression {
  schema_name: string;
  table_name: string;
  total_columns: number;
  uncompressed_cols: number;
  az64_cols: number;
  zstd_cols: number;
  lzo_cols: number;
  bytedict_cols: number;
  delta_cols: number;
  runlength_cols: number;
  raw_cols: number;
  uncompressed_pct: number;
  sort_key: string | null;
  dist_key: string | null;
}

export interface AccessPattern {
  schema_name: string;
  table_name: string;
  total_accesses: number;
  days_accessed: number;
  hours_accessed: number;
  first_access: string;
  last_access: string;
  days_since_last_access: number;
  sunday_accesses: number;
  monday_accesses: number;
  tuesday_accesses: number;
  wednesday_accesses: number;
  thursday_accesses: number;
  friday_accesses: number;
  saturday_accesses: number;
  usage_pattern: string;
}

export interface SlowQuery {
  query: number;
  schema_name: string;
  table_name: string;
  userid: number;
  username: string;
  starttime: string;
  execution_time_sec: number;
  cpu_time_sec: number;
  query_blocks_read: number;
  query_temp_blocks_to_disk: number;
  rows_scanned: number;
  rows_pre_filter: number;
  filter_efficiency_pct: number;
  query_preview: string;
}

export interface UnusedTable {
  schema_name: string;
  table_name: string;
  column_count: number;
  has_sort_key: number;
  has_dist_key: number;
  last_access: string | null;
  last_accessed: string;
  status: string;
}

export interface DiskQuery {
  query: number;
  schema_name: string;
  table_name: string;
  starttime: string;
  execution_time_sec: number;
  query_temp_blocks_to_disk: number;
  spilled_mb: number;
  rows_scanned: number;
  query_blocks_read: number;
  query_preview: string;
}

export interface TableSize {
  schema_name: string;
  table_name: string;
  estimated_rows: number;
  estimated_rows_pre_filter: number;
  estimated_mb: number;
  estimated_gb: number;
  last_full_scan: string;
}

export interface UserActivity {
  username: string;
  schema_name: string;
  table_name: string;
  access_count: number;
  unique_queries: number;
  first_access: string;
  last_access: string;
}

export interface TableDistribution {
  schema_name: string;
  table_name: string;
  dist_key_column: string | null;
  dist_style: string;
  total_columns: number;
  sort_key_column: string | null;
  sort_key_position: number | null;
  distribution_notes: string;
}

export interface FilterEffectiveness {
  schema_name: string;
  table_name: string;
  scan_count: number;
  total_rows_returned: number;
  total_rows_scanned: number;
  avg_filter_efficiency_pct: number;
  min_filter_efficiency_pct: number;
  max_filter_efficiency_pct: number;
  full_table_scans: number;
  full_scan_pct: number;
  filter_assessment: string;
}

export interface QueryFrequency {
  hour_of_day: number;
  total_scans: number;
  unique_queries: number;
  unique_tables: number;
  avg_rows_scanned: number;
  total_gb_scanned: number;
}

// =============================================================================
// Query Plan Types
// =============================================================================

export interface RecentQuery {
  query_id: number;
  query_text: string;
  user_name: string;
  start_time: string;
  end_time: string;
  execution_seconds: number;
  aborted: number;
  label: string | null;
}

export interface QueryPlanNode {
  node_id: number;
  parent_id: number;
  plan_node: string;
  info: string;
}
