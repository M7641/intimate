// Core types
export interface SnowflakeInfo {
  database: string;
}

export interface ApiResponse<T> {
  data: T[];
  columns: string[];
}

// =============================================================================
// Cost Types
// =============================================================================

export interface ExpensiveQuery {
  query_id: string;
  query_type: string;
  user_name: string;
  warehouse_name: string;
  warehouse_size: string;
  execution_seconds: number;
  total_elapsed_seconds: number;
  queued_seconds: number;
  queued_overload_seconds: number;
  queued_provisioning_seconds: number;
  gb_scanned: number;
  rows_produced: number | null;
  cloud_services_credits: number;
  estimated_credits: number;
  start_time: string;
  query_preview: string;
  query_text: string;
}

export interface CostByQueryType {
  query_type: string;
  query_count: number;
  unique_users: number;
  total_execution_hours: number;
  avg_execution_seconds: number;
  total_tb_scanned: number;
  cloud_services_credits: number;
  estimated_compute_credits: number;
}

export interface CostByTable {
  target_table: string;
  write_count: number;
  total_rows_created: number | null;
  avg_rows_created: number | null;
  total_gb_scanned: number;
  total_elapsed_seconds: number;
  total_credits: number;
}

// =============================================================================
// Performance Types
// =============================================================================

export interface WarehouseUtilization {
  warehouse_name: string;
  warehouse_size: string;
  query_count: number;
  unique_users: number;
  avg_execution_seconds: number;
  p50_execution_seconds: number;
  p95_execution_seconds: number;
  p99_execution_seconds: number;
  avg_queue_seconds: number;
  total_tb_scanned: number;
  sizing_recommendation: string;
}

export interface QueueAnalysis {
  warehouse_name: string;
  query_date: string;
  hour_of_day: number;
  query_count: number;
  avg_queue_seconds: number;
  max_queue_seconds: number;
  avg_overload_queue_seconds: number;
  queries_queued_5s_plus: number;
  queries_queued_30s_plus: number;
  pct_queued_5s_plus: number;
}

export interface CompilationAnalysis {
  warehouse_name: string;
  query_type: string;
  query_count: number;
  avg_compilation_seconds: number;
  avg_execution_seconds: number;
  avg_total_seconds: number;
  compilation_pct_of_total: number;
  p95_compilation_seconds: number;
  queries_compile_bound: number;
  pct_compile_bound: number;
}

// =============================================================================
// Storage Types
// =============================================================================

export interface TableStorage {
  database_name: string;
  schema_name: string;
  table_name: string;
  row_count: number | null;
  size_mb: number;
  size_gb: number;
  retention_time: number;
  table_created: string;
  last_altered: string;
  table_type: string;
  clustering_key: string | null;
  is_transient: string;
  auto_clustering_on: string;
}

export interface CreditsByDay {
  day: string;
  query_count: number;
  total_credits: number;
  total_elapsed_seconds: number;
}

export interface CreditsByTag {
  query_tag: string;
  tag_source: string | null;
  tag_owner: string | null;
  tag_env: string | null;
  query_count: number;
  total_credits: number;
  avg_credits: number;
  total_elapsed_seconds: number;
}

export interface FailedQuery {
  error_code: string | null;
  error_message: string | null;
  query_type: string;
  user_name: string;
  warehouse_name: string | null;
  failure_count: number;
  first_failure: string;
  last_failure: string;
}

export type MatchMode = "exact" | "fuzzy";

export interface RepeatedQuery {
  query_hash: string;
  query_preview: string;
  query_text: string;
  query_type: string;
  execution_count: number;
  unique_users: number;
  warehouses_used: number;
  avg_execution_seconds: number;
  total_execution_seconds: number;
  total_gb_scanned: number;
  estimated_total_credits: number;
}

export interface QueryFrequency {
  hour_of_day: number;
  total_queries: number;
  unique_queries: number;
  unique_users: number;
  warehouses_used: number;
  avg_execution_seconds: number;
  total_tb_scanned: number;
  cloud_services_credits: number;
}

// =============================================================================
// Query Plan Types
// =============================================================================

export interface RecentQuery {
  query_id: string;
  query_type: string;
  query_text: string;
  user_name: string;
  warehouse_name: string;
  warehouse_size: string;
  execution_status: string;
  execution_seconds: number;
  total_elapsed_seconds: number;
  mb_scanned: number;
  rows_produced: number | null;
  compilation_seconds: number;
  start_time: string;
  end_time: string;
}

export interface QueryPlanNode {
  step_id: number;
  operator_id: number;
  parent_operators: string | null;
  operator_type: string;
  operator_statistics: string | null;
  execution_time_breakdown: string | null;
  operator_attributes: string | null;
}
