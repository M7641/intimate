import { useQuery } from "@tanstack/react-query";
import type {
  SnowflakeInfo,
  ApiResponse,
  ExpensiveQuery,
  CostByQueryType,
  CostByTable,
  CreditsByDay,
  CreditsByTag,
  WarehouseUtilization,
  QueueAnalysis,
  CompilationAnalysis,
  TableStorage,
  QueryFrequency,
  FailedQuery,
  RepeatedQuery,
  RecentQuery,
  QueryPlanNode,
  MatchMode,
} from "@/types/snowflake";

async function fetchApi<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`API error: ${res.status}`);
  }
  return res.json();
}

// =============================================================================
// Core
// =============================================================================

export function useSnowflakeInfo() {
  return useQuery({
    queryKey: ["snowflake", "info"],
    queryFn: () => fetchApi<SnowflakeInfo>("/api/info"),
  });
}

// =============================================================================
// Cost Hooks
// =============================================================================

export function useExpensiveQueries(days = 7, topN = 50, search?: string) {
  const searchParam = search ? `&search=${encodeURIComponent(search)}` : "";
  return useQuery({
    queryKey: ["snowflake", "expensive-queries", days, topN, search],
    queryFn: () =>
      fetchApi<ApiResponse<ExpensiveQuery>>(
        `/api/expensive-queries?days=${days}&top_n=${topN}${searchParam}`
      ),
  });
}

export function useCostByTable(days = 7, topN = 100, search?: string) {
  const searchParam = search ? `&search=${encodeURIComponent(search)}` : "";
  return useQuery({
    queryKey: ["snowflake", "cost-by-table", days, topN, search],
    queryFn: () =>
      fetchApi<ApiResponse<CostByTable>>(
        `/api/cost-by-table?days=${days}&top_n=${topN}${searchParam}`
      ),
  });
}

export function useCostByQueryType(days = 30) {
  return useQuery({
    queryKey: ["snowflake", "cost-by-query-type", days],
    queryFn: () =>
      fetchApi<ApiResponse<CostByQueryType>>(
        `/api/cost-by-query-type?days=${days}`
      ),
  });
}

// =============================================================================
// Performance Hooks
// =============================================================================

export function useWarehouseUtilization(days = 7) {
  return useQuery({
    queryKey: ["snowflake", "warehouse-utilization", days],
    queryFn: () =>
      fetchApi<ApiResponse<WarehouseUtilization>>(
        `/api/warehouse-utilization?days=${days}`
      ),
  });
}

export function useQueueAnalysis(days = 7) {
  return useQuery({
    queryKey: ["snowflake", "queue-analysis", days],
    queryFn: () =>
      fetchApi<ApiResponse<QueueAnalysis>>(
        `/api/queue-analysis?days=${days}`
      ),
  });
}

export function useCompilationAnalysis(days = 7) {
  return useQuery({
    queryKey: ["snowflake", "compilation-analysis", days],
    queryFn: () =>
      fetchApi<ApiResponse<CompilationAnalysis>>(
        `/api/compilation-analysis?days=${days}`
      ),
  });
}

// =============================================================================
// Storage Hooks
// =============================================================================

export function useTableStorage(database?: string, schema?: string) {
  const params = new URLSearchParams();
  if (database) params.append("database", database);
  if (schema) params.append("schema", schema);
  const queryString = params.toString();

  return useQuery({
    queryKey: ["snowflake", "table-storage", database, schema],
    queryFn: () =>
      fetchApi<ApiResponse<TableStorage>>(
        `/api/table-storage${queryString ? `?${queryString}` : ""}`
      ),
  });
}

export function useCreditsByDay(days = 7) {
  return useQuery({
    queryKey: ["snowflake", "credits-by-day", days],
    queryFn: () =>
      fetchApi<ApiResponse<CreditsByDay>>(
        `/api/credits-by-day?days=${days}`
      ),
  });
}

export function useCreditsByTag(days = 7) {
  return useQuery({
    queryKey: ["snowflake", "credits-by-tag", days],
    queryFn: () =>
      fetchApi<ApiResponse<CreditsByTag>>(
        `/api/credits-by-tag?days=${days}`
      ),
  });
}

export function useFailedQueries(days = 7) {
  return useQuery({
    queryKey: ["snowflake", "failed-queries", days],
    queryFn: () =>
      fetchApi<ApiResponse<FailedQuery>>(
        `/api/failed-queries?days=${days}`
      ),
  });
}

export function useRepeatedQueries(
  days = 7,
  minCount = 5,
  mode: MatchMode = "fuzzy"
) {
  return useQuery({
    queryKey: ["snowflake", "repeated-queries", days, minCount, mode],
    queryFn: () =>
      fetchApi<ApiResponse<RepeatedQuery>>(
        `/api/repeated-queries?days=${days}&min_count=${minCount}&mode=${mode}`
      ),
  });
}

export function useQueryFrequency(days = 7) {
  return useQuery({
    queryKey: ["snowflake", "query-frequency", days],
    queryFn: () =>
      fetchApi<ApiResponse<QueryFrequency>>(
        `/api/query-frequency?days=${days}`
      ),
  });
}

// =============================================================================
// Query Plan Hooks
// =============================================================================

export function useRecentQueries(days = 7, limit = 50) {
  return useQuery({
    queryKey: ["snowflake", "recent-queries", days, limit],
    queryFn: () =>
      fetchApi<ApiResponse<RecentQuery>>(
        `/api/recent-queries?days=${days}&limit=${limit}`
      ),
  });
}

export function useQueryPlan(queryId: string | null) {
  return useQuery({
    queryKey: ["snowflake", "query-plan", queryId],
    queryFn: () =>
      fetchApi<ApiResponse<QueryPlanNode>>(
        `/api/query-plan?query_id=${queryId}`
      ),
    enabled: !!queryId,
  });
}
