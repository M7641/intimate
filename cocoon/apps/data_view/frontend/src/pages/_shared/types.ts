// ── Table Info types ──────────────────────────────────────────────────

/** A table's most recent load, for the home "recently updated" overview. */
export interface RecentLoad {
  schema: string;
  table_name: string;
  last_loaded: string | null;
}

export interface ColumnDetail {
  column_name: string;
  data_type: string;
  ordinal_position: number;
  is_nullable: boolean;
  // Redshift
  encoding?: string;
  distkey?: boolean;
  sortkey?: number;
  // Snowflake
  character_maximum_length?: number;
  numeric_precision?: number;
  numeric_scale?: number;
  column_default?: string;
  column_comment?: string;
}

export interface RedshiftTableStorage {
  size_mb?: number;
  tbl_rows?: number;
  unsorted?: number;
  stats_off?: number;
  skew_rows?: number;
  skew_sortkey1?: number;
  encoded?: string;
  diststyle?: string;
  sortkey1?: string;
  sortkey_num?: number;
  pct_used?: number;
}

export interface SnowflakeTableDetails {
  row_count?: number;
  bytes?: number;
  retention_time?: string;
  created?: string;
  last_altered?: string;
  auto_clustering_on?: boolean;
  cluster_by?: string;
  is_transient?: boolean;
  table_type?: string;
  table_comment?: string;
}

export interface TableMeta {
  table_name: string;
  column_count: number;
  total_rows?: number;
  snapshot_count?: number;
  columns: ColumnDetail[];
  redshift_storage?: RedshiftTableStorage;
  snowflake_details?: SnowflakeTableDetails;
}

// ── Data Explorer types ───────────────────────────────────────────────

export type SnapshotTimestamp = {
  load_timestamp: string;
};

export type SnapshotData = Record<string, unknown>;

export interface TimestampRowCount {
  load_timestamp: string;
  row_count: number;
}

export type FilterOperator = "equals" | "contains" | "startsWith" | "endsWith";

// From backend /columns endpoint
export interface ColumnInfo {
  column_name: string;
  data_type: string;
}

export interface ColumnFilter {
  id: string;
  column: string;
  operator: FilterOperator;
  value: string;
}

export interface ColumnMeta {
  key: string;
  displayName: string;
  uniqueValues: string[];
  cardinality: number;
}

export interface ColumnStats {
  column_name: string;
  data_type: string;
  is_numeric: boolean;
  total_count: number;
  null_count: number;
  null_pct: number;
  distinct_count: number;
  cardinality_pct: number;
  min_val: number | null;
  max_val: number | null;
  mean_val: number | null;
  median_val: number | null;
  stddev_val: number | null;
  p25: number | null;
  p75: number | null;
}

export interface ValueDistribution {
  column_name: string;
  data_type: string;
  is_numeric: boolean;
  distribution: ValueDistributionItem[];
}

// ── Catalogue map (whole-database view) ───────────────────────────────

export interface CatalogueNode {
  table_name: string;
  column_count: number;
  key_columns: string[];
}

/** How an edge was discovered: a declared FK, or a naming-convention guess. */
export type EdgeKind = "declared" | "inferred";

/** One column pairing on an edge (names match for inferred links). */
export interface EdgeColumn {
  source_column: string;
  target_column: string;
}

export interface CatalogueEdge {
  source: string;
  target: string;
  kind: EdgeKind;
  links: EdgeColumn[];
}

export interface OmittedColumn {
  column: string;
  table_count: number;
}

export interface Catalogue {
  schema: string;
  nodes: CatalogueNode[];
  edges: CatalogueEdge[];
  omitted_columns: OmittedColumn[];
}

export interface LinkOverlap {
  source_table: string;
  target_table: string;
  source_column: string;
  target_column: string;
  source_distinct: number;
  target_distinct: number;
  overlap: number;
  source_coverage_pct: number;
  target_coverage_pct: number;
}

// ── Schema health dashboard ───────────────────────────────────────────

export interface TableHealth {
  table_name: string;
  last_loaded: string | null;
  snapshot_count: number;
  current_rows: number;
  previous_rows: number | null;
  row_delta_pct: number | null;
}

export interface SchemaHealth {
  schema: string;
  tables: TableHealth[];
}

export interface ValueDistributionItem {
  value: string | null;
  bucket: number | null;
  bin_min: number | null;
  bin_max: number | null;
  count: number;
}
