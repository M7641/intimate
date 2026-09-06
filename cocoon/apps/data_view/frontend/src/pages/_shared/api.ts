import { useQuery, keepPreviousData } from "@tanstack/react-query";
import type {
  SnapshotTimestamp,
  SnapshotData,
  ColumnFilter,
  ColumnInfo,
  TimestampRowCount,
  ColumnStats,
  ValueDistribution,
  TableMeta,
  RecentLoad,
  Catalogue,
  LinkOverlap,
  SchemaHealth,
} from "./types";

export function useDataExplorerSchemas() {
  return useQuery<string[]>({
    queryKey: ["dataExplorerSchemas"],
    queryFn: async () => {
      const response = await fetch("/api/data_view/schemas");
      if (!response.ok) throw new Error("Failed to fetch schemas");
      return response.json();
    },
  });
}

export function useDataExplorerTables(schema: string) {
  return useQuery<string[]>({
    queryKey: ["dataExplorerTables", schema],
    queryFn: async () => {
      const response = await fetch(
        `/api/data_view/tables?schema=${encodeURIComponent(schema)}`,
      );
      if (!response.ok) throw new Error("Failed to fetch tables");
      return response.json();
    },
    enabled: !!schema,
  });
}

export function useRecentLoads(schema: string, limit = 8) {
  return useQuery<RecentLoad[]>({
    queryKey: ["recentLoads", schema, limit],
    queryFn: async () => {
      const response = await fetch(
        `/api/data_view/recent_loads?schema=${encodeURIComponent(schema)}&limit=${limit}`,
      );
      if (!response.ok) throw new Error("Failed to fetch recent loads");
      return response.json();
    },
    enabled: !!schema,
  });
}

export function useDataExplorerTimestamps(
  schema: string,
  selectedTable: string,
) {
  return useQuery<SnapshotTimestamp[]>({
    queryKey: ["dataExplorerTimestamps", schema, selectedTable],
    queryFn: async () => {
      const response = await fetch(
        `/api/data_view/timestamps/${selectedTable}?schema=${encodeURIComponent(schema)}`,
      );
      if (!response.ok) throw new Error("Failed to fetch timestamps");
      return response.json();
    },
    enabled: !!schema && !!selectedTable,
    // Keep the prior table's timestamps on a switch so the control doesn't blank
    // (and the page doesn't suspend). See docs/data-fetching-and-reactivity.md.
    placeholderData: keepPreviousData,
  });
}

export function useDataExplorerColumns(schema: string, selectedTable: string) {
  return useQuery<ColumnInfo[]>({
    queryKey: ["dataExplorerColumns", schema, selectedTable],
    queryFn: async () => {
      const response = await fetch(
        `/api/data_view/columns/${selectedTable}?schema=${encodeURIComponent(schema)}`,
      );
      if (!response.ok) throw new Error("Failed to fetch columns");
      return response.json();
    },
    enabled: !!schema && !!selectedTable,
    placeholderData: keepPreviousData,
  });
}

export function useDataExplorerColumnValues(
  schema: string,
  selectedTable: string,
  columnName: string,
  selectedTimestamp?: string,
) {
  return useQuery<string[]>({
    queryKey: [
      "dataExplorerColumnValues",
      schema,
      selectedTable,
      columnName,
      selectedTimestamp,
    ],
    queryFn: async () => {
      const params = new URLSearchParams({ schema });
      if (selectedTimestamp) {
        params.append("load_timestamp", selectedTimestamp);
      }
      const url = `/api/data_view/column_values/${selectedTable}/${columnName}?${params}`;
      const response = await fetch(url);
      if (!response.ok) throw new Error("Failed to fetch column values");
      return response.json();
    },
    enabled: !!schema && !!selectedTable && !!columnName,
  });
}

export function useDataExplorerData(
  schema: string,
  selectedTable: string,
  selectedTimestamp: string,
  filters: ColumnFilter[] = [],
) {
  // Only include active filters (those with column and value set)
  const activeFilters = filters.filter((f) => f.column && f.value);

  return useQuery<SnapshotData[]>({
    queryKey: [
      "dataExplorerData",
      schema,
      selectedTable,
      selectedTimestamp,
      activeFilters,
    ],
    queryFn: async () => {
      const params = new URLSearchParams({ schema, limit: "1000" });
      if (selectedTimestamp) {
        params.append("load_timestamp", selectedTimestamp);
      }
      if (activeFilters.length > 0) {
        params.append("filters", JSON.stringify(activeFilters));
      }
      const response = await fetch(
        `/api/data_view/data/${selectedTable}?${params}`,
      );
      if (!response.ok) throw new Error("Failed to fetch data");
      return response.json();
    },
    enabled: !!schema && !!selectedTable,
    // Keep the previous table's rows on screen while the next load runs.
    placeholderData: keepPreviousData,
  });
}

export function useDataExplorerRowCounts(
  schema: string,
  selectedTable: string,
) {
  return useQuery<TimestampRowCount[]>({
    queryKey: ["dataExplorerRowCounts", schema, selectedTable],
    queryFn: async () => {
      const response = await fetch(
        `/api/data_view/row_counts/${selectedTable}?schema=${encodeURIComponent(schema)}`,
      );
      if (!response.ok) throw new Error("Failed to fetch row counts");
      return response.json();
    },
    enabled: !!schema && !!selectedTable,
    placeholderData: keepPreviousData,
  });
}

/**
 * Query options for one column's stats at a given load time. Shared between the
 * single-result `useColumnStats` hook and `useQueries` in the compare view, so
 * both build the identical query key and only fetch for a real timestamp.
 */
export function columnStatsQueryOptions(
  schema: string,
  selectedTable: string,
  columnName: string,
  selectedTimestamp?: string,
) {
  return {
    queryKey: [
      "columnStats",
      schema,
      selectedTable,
      columnName,
      selectedTimestamp,
    ],
    queryFn: async (): Promise<ColumnStats> => {
      const params = new URLSearchParams({ schema });
      if (selectedTimestamp) {
        params.append("load_timestamp", selectedTimestamp);
      }
      const url = `/api/data_view/column_stats/${selectedTable}/${columnName}?${params}`;
      const response = await fetch(url);
      if (!response.ok) throw new Error("Failed to fetch column stats");
      return response.json();
    },
    enabled: !!schema && !!selectedTable && !!columnName,
    placeholderData: keepPreviousData,
  };
}

export function useColumnStats(
  schema: string,
  selectedTable: string,
  columnName: string,
  selectedTimestamp?: string,
) {
  return useQuery<ColumnStats>(
    columnStatsQueryOptions(schema, selectedTable, columnName, selectedTimestamp),
  );
}

export function useTableMeta(schema: string, selectedTable: string) {
  return useQuery<TableMeta>({
    queryKey: ["tableMeta", schema, selectedTable],
    queryFn: async () => {
      const response = await fetch(
        `/api/data_view/table_meta/${selectedTable}?schema=${encodeURIComponent(schema)}`,
      );
      if (!response.ok) throw new Error("Failed to fetch table metadata");
      return response.json();
    },
    enabled: !!schema && !!selectedTable,
    placeholderData: keepPreviousData,
  });
}

/** Query options for one column's value distribution at a given load time. */
export function valueDistributionQueryOptions(
  schema: string,
  selectedTable: string,
  columnName: string,
  selectedTimestamp?: string,
) {
  return {
    queryKey: [
      "valueDistribution",
      schema,
      selectedTable,
      columnName,
      selectedTimestamp,
    ],
    queryFn: async (): Promise<ValueDistribution> => {
      const params = new URLSearchParams({ schema });
      if (selectedTimestamp) {
        params.append("load_timestamp", selectedTimestamp);
      }
      const url = `/api/data_view/value_distribution/${selectedTable}/${columnName}?${params}`;
      const response = await fetch(url);
      if (!response.ok) throw new Error("Failed to fetch value distribution");
      return response.json();
    },
    enabled: !!schema && !!selectedTable && !!columnName,
    placeholderData: keepPreviousData,
  };
}

export function useValueDistribution(
  schema: string,
  selectedTable: string,
  columnName: string,
  selectedTimestamp?: string,
) {
  return useQuery<ValueDistribution>(
    valueDistributionQueryOptions(
      schema,
      selectedTable,
      columnName,
      selectedTimestamp,
    ),
  );
}

// ── Catalogue map ─────────────────────────────────────────────────────

/** The whole-database catalogue graph for a schema (cheap, metadata-only). */
export function useCatalogue(schema: string) {
  return useQuery<Catalogue>({
    queryKey: ["catalogue", schema],
    queryFn: async () => {
      const response = await fetch(
        `/api/data_view/catalogue?schema=${encodeURIComponent(schema)}`,
      );
      if (!response.ok) throw new Error("Failed to fetch catalogue");
      return response.json();
    },
    enabled: !!schema,
    placeholderData: keepPreviousData,
  });
}

/** One link's value-overlap validation (heavy). Pass null to leave it idle. */
export interface OverlapRequest {
  schema: string;
  sourceTable: string;
  targetTable: string;
  sourceColumn: string;
  targetColumn: string;
}

export function useLinkOverlap(request: OverlapRequest | null) {
  return useQuery<LinkOverlap>({
    // Each distinct link caches independently, so re-clicking is instant.
    queryKey: ["linkOverlap", request],
    queryFn: async () => {
      const r = request!;
      const params = new URLSearchParams({
        schema: r.schema,
        source_table: r.sourceTable,
        target_table: r.targetTable,
        source_column: r.sourceColumn,
        target_column: r.targetColumn,
      });
      const response = await fetch(`/api/data_view/link_overlap?${params}`);
      if (!response.ok) throw new Error("Failed to validate link");
      return response.json();
    },
    enabled: !!request,
  });
}

// ── Schema health ─────────────────────────────────────────────────────

/** Per-table freshness + volume-trend summary for a whole schema (heavy). */
export function useSchemaHealth(schema: string) {
  return useQuery<SchemaHealth>({
    queryKey: ["schemaHealth", schema],
    queryFn: async () => {
      const response = await fetch(
        `/api/data_view/schema_health?schema=${encodeURIComponent(schema)}`,
      );
      if (!response.ok) throw new Error("Failed to fetch schema health");
      return response.json();
    },
    enabled: !!schema,
    placeholderData: keepPreviousData,
  });
}
