import { useQuery } from "@tanstack/react-query";
import type {
  RedshiftInfo,
  ApiResponse,
  TableScan,
  QueryPerformance,
  TableCompression,
  AccessPattern,
  SlowQuery,
  UnusedTable,
  DiskQuery,
  TableSize,
  StorageTable,
  UserActivity,
  TableDistribution,
  FilterEffectiveness,
  QueryFrequency,
  RecentQuery,
  QueryPlanNode,
} from "@/types/redshift";

async function fetchApi<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`API error: ${res.status}`);
  }
  return res.json();
}

export function useRedshiftInfo() {
  return useQuery({
    queryKey: ["redshift", "info"],
    queryFn: () => fetchApi<RedshiftInfo>("/api/info"),
  });
}

export function useTableScans(days = 30, schema?: string) {
  return useQuery({
    queryKey: ["redshift", "table-scans", days, schema],
    queryFn: () =>
      fetchApi<ApiResponse<TableScan>>(
        `/api/table-scans?days=${days}${schema ? `&schema=${schema}` : ""}`
      ),
  });
}

export function useQueryPerformance(days = 30) {
  return useQuery({
    queryKey: ["redshift", "query-performance", days],
    queryFn: () =>
      fetchApi<ApiResponse<QueryPerformance>>(
        `/api/query-performance?days=${days}`
      ),
  });
}

export function useCompression(schema?: string) {
  return useQuery({
    queryKey: ["redshift", "compression", schema],
    queryFn: () =>
      fetchApi<ApiResponse<TableCompression>>(
        `/api/compression${schema ? `?schema=${schema}` : ""}`
      ),
  });
}

export function useAccessPatterns(days = 30) {
  return useQuery({
    queryKey: ["redshift", "access-patterns", days],
    queryFn: () =>
      fetchApi<ApiResponse<AccessPattern>>(
        `/api/access-patterns?days=${days}`
      ),
  });
}

export function useSlowQueries(minSeconds = 10, days = 7) {
  return useQuery({
    queryKey: ["redshift", "slow-queries", minSeconds, days],
    queryFn: () =>
      fetchApi<ApiResponse<SlowQuery>>(
        `/api/slow-queries?min_seconds=${minSeconds}&days=${days}`
      ),
  });
}

export function useUnusedTables(daysThreshold = 30) {
  return useQuery({
    queryKey: ["redshift", "unused-tables", daysThreshold],
    queryFn: () =>
      fetchApi<ApiResponse<UnusedTable>>(
        `/api/unused-tables?days_threshold=${daysThreshold}`
      ),
  });
}

export function useDiskQueries(days = 7) {
  return useQuery({
    queryKey: ["redshift", "disk-queries", days],
    queryFn: () =>
      fetchApi<ApiResponse<DiskQuery>>(`/api/disk-queries?days=${days}`),
  });
}

export function useTableSizes(schema?: string) {
  return useQuery({
    queryKey: ["redshift", "table-sizes", schema],
    queryFn: () =>
      fetchApi<ApiResponse<TableSize>>(
        `/api/table-sizes${schema ? `?schema=${schema}` : ""}`
      ),
  });
}

export function useStorage(schema?: string) {
  return useQuery({
    queryKey: ["redshift", "storage", schema],
    queryFn: () =>
      fetchApi<ApiResponse<StorageTable>>(
        `/api/storage${schema ? `?schema=${schema}` : ""}`
      ),
  });
}

export function useUserActivity(days = 30) {
  return useQuery({
    queryKey: ["redshift", "user-activity", days],
    queryFn: () =>
      fetchApi<ApiResponse<UserActivity>>(
        `/api/user-activity?days=${days}`
      ),
  });
}

export function useDistribution() {
  return useQuery({
    queryKey: ["redshift", "distribution"],
    queryFn: () =>
      fetchApi<ApiResponse<TableDistribution>>("/api/distribution"),
  });
}

export function useFilterEffectiveness(days = 7, minScans = 10) {
  return useQuery({
    queryKey: ["redshift", "filter-effectiveness", days, minScans],
    queryFn: () =>
      fetchApi<ApiResponse<FilterEffectiveness>>(
        `/api/filter-effectiveness?days=${days}&min_scans=${minScans}`
      ),
  });
}

export function useQueryFrequency(days = 7) {
  return useQuery({
    queryKey: ["redshift", "query-frequency", days],
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
    queryKey: ["redshift", "recent-queries", days, limit],
    queryFn: () =>
      fetchApi<ApiResponse<RecentQuery>>(
        `/api/recent-queries?days=${days}&limit=${limit}`
      ),
  });
}

export function useQueryPlan(queryId: number | null) {
  return useQuery({
    queryKey: ["redshift", "query-plan", queryId],
    queryFn: () =>
      fetchApi<ApiResponse<QueryPlanNode>>(
        `/api/query-plan?query_id=${queryId}`
      ),
    enabled: queryId !== null,
  });
}
