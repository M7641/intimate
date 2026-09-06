"use no memo";

import React from "react";
import { useSearch, useNavigate } from "@tanstack/react-router";

import { SCHEMA_STORAGE_KEY } from "@/pages/_shared/prefs";
import {
  Chart as ChartJS,
  Title,
  Tooltip,
  Legend,
  Colors,
  BarController,
  BarElement,
  CategoryScale,
  LinearScale,
  type ChartOptions,
  type TooltipItem,
} from "chart.js";
import { Bar } from "react-chartjs-2";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";

import DataLoading from "@/components/DataLoad";
import NoDataAvailable from "@/components/NoDataAvailable";
import MTable from "@/components/MTable";
import StatCard from "@/components/StatCard";
import FetchingIndicator from "@/components/FetchingIndicator";
import { useTheme } from "@/components/themeProvider";
import { readChartPalette } from "@/lib/chartColors";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  formatNumber,
  formatBytes,
  formatTimestampShort,
  formatTableName,
  formatColumnName,
} from "@/lib/format";

import {
  useDataExplorerSchemas,
  useDataExplorerTables,
  useDataExplorerRowCounts,
  useTableMeta,
} from "@/pages/_shared/api";
import type {
  ColumnDetail,
  RedshiftTableStorage,
  SnowflakeTableDetails,
  TableMeta,
} from "@/pages/_shared/types";

// Chart.js components must be registered once before any chart renders.
ChartJS.register(
  Title,
  Tooltip,
  Legend,
  Colors,
  BarController,
  BarElement,
  CategoryScale,
  LinearScale,
);

// ── Overview Cards ────────────────────────────────────────────────────

function OverviewCards({ meta }: { meta: TableMeta }) {
  return (
    <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
      <StatCard label="Columns" value={formatNumber(meta.column_count)} />
      <StatCard label="Rows (Latest)" value={formatNumber(meta.total_rows)} />
      <StatCard
        label="Snapshots"
        value={formatNumber(meta.snapshot_count)}
        hint="Number of point-in-time loads captured for this table."
      />
    </div>
  );
}

// ── Columns Table ─────────────────────────────────────────────────────

const columnHelper = createColumnHelper<ColumnDetail>();

function ColumnsTable({
  columns,
  meta,
  onColumnClick,
}: {
  columns: ColumnDetail[];
  meta: TableMeta;
  /** Open Column Analysis for the clicked column. */
  onColumnClick?: (columnName: string) => void;
}) {
  const isRedshift = meta.redshift_storage != null;
  const isSnowflake = meta.snowflake_details != null;

  const tableColumns = React.useMemo(() => {
    const base: ColumnDef<ColumnDetail, any>[] = [
      columnHelper.accessor("ordinal_position", {
        header: "#",
        size: 50,
      }),
      columnHelper.accessor("column_name", {
        header: "Column",
        size: 200,
        cell: (info) => formatColumnName(info.getValue()),
      }),
      columnHelper.accessor("data_type", {
        header: "Type",
        size: 140,
      }),
      columnHelper.accessor("is_nullable", {
        header: "Nullable",
        size: 80,
        cell: (info) => (info.getValue() ? "Yes" : "No"),
      }),
    ];

    if (isRedshift) {
      base.push(
        columnHelper.accessor("encoding", {
          header: "Compression",
          size: 110,
          cell: (info) => info.getValue() ?? "—",
        }),
        columnHelper.accessor("distkey", {
          header: "Distributed",
          size: 90,
          cell: (info) => {
            const v = info.getValue();
            return v === true ? "Yes" : v === false ? "No" : "—";
          },
        }),
        columnHelper.accessor("sortkey", {
          header: "Sort Order",
          size: 90,
          cell: (info) => {
            const v = info.getValue();
            return v != null && v !== 0 ? String(v) : "—";
          },
        }),
      );
    }

    if (isSnowflake) {
      base.push(
        columnHelper.accessor("numeric_precision", {
          header: "Precision",
          size: 90,
          cell: (info) => info.getValue() ?? "—",
        }),
        columnHelper.accessor("numeric_scale", {
          header: "Scale",
          size: 70,
          cell: (info) => info.getValue() ?? "—",
        }),
        columnHelper.accessor("column_default", {
          header: "Default",
          size: 120,
          cell: (info) => info.getValue() ?? "—",
        }),
        columnHelper.accessor("column_comment", {
          header: "Comment",
          size: 200,
          cell: (info) => info.getValue() ?? "—",
        }),
      );
    }

    return base;
  }, [isRedshift, isSnowflake]);

  return (
    <MTable
      data={columns}
      columns={tableColumns}
      onRowClick={
        onColumnClick ? (row) => onColumnClick(row.column_name) : undefined
      }
    />
  );
}

// ── Row Counts Chart ──────────────────────────────────────────────────

function RowCountsChart({
  selectedSchema,
  selectedTable,
}: {
  selectedSchema: string;
  selectedTable: string;
}) {
  // Resolve theme palette to concrete colours (canvas can't read CSS vars) and
  // re-resolve whenever the active theme changes.
  const { theme } = useTheme();
  const colors = React.useMemo(() => readChartPalette(), [theme]);

  const { data: rowCounts, error, isFetching } = useDataExplorerRowCounts(
    selectedSchema,
    selectedTable,
  );
  // First load = nothing to show yet; a table switch keeps the prior series
  // (keepPreviousData) and updates in place.
  const firstLoad = !rowCounts && !error;

  // Cap the window to the last month. Fall back to the full series if the
  // table hasn't loaded in over a month, so the chart isn't left empty.
  const points = React.useMemo(() => {
    if (!rowCounts) return [];
    const cutoff = Date.now() - 30 * 24 * 60 * 60 * 1000;
    const lastMonth = rowCounts.filter((rc) => {
      const t = new Date(rc.load_timestamp).getTime();
      return !Number.isNaN(t) && t >= cutoff;
    });
    return lastMonth.length > 0 ? lastMonth : rowCounts;
  }, [rowCounts]);

  const chartData = React.useMemo(
    () => ({
      labels: points.map((rc) => formatTimestampShort(rc.load_timestamp)),
      datasets: [
        {
          label: "Row Count",
          data: points.map((rc) => rc.row_count),
          backgroundColor: colors.series[0],
          borderRadius: 4,
          maxBarThickness: 48,
        },
      ],
    }),
    [points, colors],
  );

  const chartOptions = React.useMemo<ChartOptions<"bar">>(
    () => ({
      responsive: true,
      maintainAspectRatio: false,
      layout: { padding: { top: 16, right: 24, left: 16, bottom: 8 } },
      plugins: {
        legend: { display: false },
        tooltip: {
          callbacks: {
            // Show the full load_timestamp on the tooltip title.
            title: (items: TooltipItem<"bar">[]) => {
              const i = items[0]?.dataIndex ?? 0;
              return points[i]?.load_timestamp ?? "";
            },
            label: (item: TooltipItem<"bar">) =>
              `Row Count: ${Number(item.parsed.y).toLocaleString()}`,
          },
        },
      },
      scales: {
        x: {
          ticks: {
            color: colors.ticks,
            font: { size: 11 },
            maxRotation: 45,
            minRotation: 45,
          },
          grid: { display: false },
          border: { color: colors.grid },
        },
        y: {
          ticks: {
            color: colors.ticks,
            font: { size: 12 },
            callback: (v: number | string) => Number(v).toLocaleString(),
          },
          grid: { color: colors.grid },
          border: { display: false },
        },
      },
    }),
    [points, colors],
  );

  if (firstLoad || error)
    return <DataLoading isPending={firstLoad} error={error} variant="chart" />;
  if (points.length === 0)
    return <NoDataAvailable message="No snapshots available" />;

  return (
    <div className="relative h-[300px]">
      <FetchingIndicator
        when={isFetching && !!rowCounts}
        className="absolute right-2 top-2 z-10"
      />
      <Bar data={chartData} options={chartOptions} />
    </div>
  );
}

// ── Redshift Storage Section ──────────────────────────────────────────

function RedshiftStorageSection({
  storage,
}: {
  storage: RedshiftTableStorage;
}) {
  return (
    <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
      <StatCard
        label="Distribution"
        value={storage.diststyle ?? "Not set"}
        hint="How the table's rows are spread across storage so queries can read them in parallel."
      />
      <StatCard
        label="Sort Order"
        value={storage.sortkey1 ?? "Not set"}
        sub={
          storage.sortkey_num != null
            ? `${storage.sortkey_num} column(s)`
            : undefined
        }
        hint="The column the table is physically ordered by; filtering on it is faster."
      />
      <StatCard
        label="Unsorted Rows"
        value={
          storage.unsorted != null
            ? `${formatNumber(storage.unsorted)}%`
            : "Not set"
        }
        hint="Share of rows not yet in sort order. Higher means slower filtered reads."
      />
      <StatCard
        label="Stats Freshness"
        value={
          storage.stats_off != null
            ? `${formatNumber(storage.stats_off)}% stale`
            : "Not set"
        }
        hint="How out of date the query planner's statistics are. Lower is better."
      />
      <StatCard
        label="Row Balance"
        value={formatNumber(storage.skew_rows)}
        hint="How evenly rows are distributed across storage. 1.0 is perfectly even."
      />
      <StatCard
        label="Compression"
        value={storage.encoded ?? "Not set"}
        hint="Whether column compression is applied to reduce stored size."
      />
      <StatCard
        label="Space Used"
        value={
          storage.pct_used != null
            ? `${formatNumber(storage.pct_used)}%`
            : "Not set"
        }
        hint="Percentage of allocated storage currently in use."
      />
      <StatCard
        label="Est. Rows"
        value={formatNumber(storage.tbl_rows)}
        hint="Approximate row count from the table's statistics."
      />
    </div>
  );
}

// ── Snowflake Details Section ─────────────────────────────────────────

function SnowflakeDetailsSection({
  details,
}: {
  details: SnowflakeTableDetails;
}) {
  return (
    <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
      <StatCard label="Row Count" value={formatNumber(details.row_count)} />
      <StatCard label="Size" value={formatBytes(details.bytes)} />
      <StatCard
        label="Table Type"
        value={details.table_type ?? "Not set"}
        hint="Whether this is a standard, temporary, or external table."
      />
      <StatCard
        label="History Kept"
        value={details.retention_time ?? "Not set"}
        hint="How long past versions are retained for point-in-time (time-travel) queries."
      />
      <StatCard label="Created" value={details.created ?? "Not set"} />
      <StatCard
        label="Last Altered"
        value={details.last_altered ?? "Not set"}
      />
      <StatCard
        label="Clustering"
        value={details.cluster_by ?? "Not set"}
        sub={details.auto_clustering_on ? "Auto-clustering ON" : undefined}
        hint="Column(s) used to co-locate related rows so reads scan less data."
      />
      <StatCard
        label="Transient"
        value={details.is_transient ? "Yes" : "No"}
        hint="Transient tables skip long-term backup, trading durability for lower cost."
      />
      {details.table_comment && (
        <div className="col-span-full">
          <StatCard label="Comment" value={details.table_comment} />
        </div>
      )}
    </div>
  );
}

// ── Section Header ────────────────────────────────────────────────────

function SectionHeader({ title }: { title: string }) {
  return (
    <h2 className="text-sm font-semibold text-muted-foreground tracking-wider uppercase border-b pb-1">
      {title}
    </h2>
  );
}

// ── Main Page ─────────────────────────────────────────────────────────

export default function TableInfoPage() {
  // Deep link from the home "recently updated" list: ?schema=&table=
  const search = useSearch({ strict: false }) as {
    schema?: string;
    table?: string;
  };
  // Seed from the URL first (a deep link wins), then the remembered choice.
  const [selectedSchema, setSelectedSchema] = React.useState<string>(
    () => search.schema ?? localStorage.getItem(SCHEMA_STORAGE_KEY) ?? "",
  );
  const [selectedTable, setSelectedTable] = React.useState<string>(
    search.table ?? "",
  );
  const navigate = useNavigate();

  // Remember the schema across visits.
  React.useEffect(() => {
    if (selectedSchema) localStorage.setItem(SCHEMA_STORAGE_KEY, selectedSchema);
  }, [selectedSchema]);

  const { data: schemas, isPending: schemasLoading } = useDataExplorerSchemas();

  // Auto-select schema: prefer "stage", else first available. Also corrects a
  // remembered schema that no longer exists in the available set.
  React.useEffect(() => {
    if (schemas && schemas.length > 0) {
      if (!selectedSchema || !schemas.includes(selectedSchema)) {
        setSelectedSchema(schemas.includes("stage") ? "stage" : schemas[0]);
      }
    }
  }, [schemas, selectedSchema]);

  // Clearing the table when the schema changes is done in the schema Select's
  // onChange (a real user action) — NOT in an effect. An effect watching
  // selectedSchema fires on mount and, under StrictMode's double-invoke, wipes a
  // deep-linked ?table= before the data ever loads.

  // Follow deep-link changes after mount: useState only seeds the selection
  // once, so a fresh ?schema=&table= (clicking a different catalogue node, or
  // arriving while this page is still mounted) wouldn't otherwise move the
  // selects. Programmatic sets here don't reset the table — only user-driven
  // schema changes do.
  React.useEffect(() => {
    if (search.schema && search.schema !== selectedSchema) {
      setSelectedSchema(search.schema);
    }
    if (search.table && search.table !== selectedTable) {
      setSelectedTable(search.table);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [search.schema, search.table]);

  const { data: tables, isPending: tablesLoading } =
    useDataExplorerTables(selectedSchema);
  const {
    data: tableMeta,
    error: metaError,
    isFetching: metaFetching,
    refetch: refetchMeta,
  } = useTableMeta(selectedSchema, selectedTable);
  // First load = nothing to show yet. A table switch keeps the prior metadata on
  // screen (keepPreviousData) and updates in place.
  const tableMetaFirstLoad = !tableMeta && !metaError;

  React.useEffect(() => {
    if (tables && tables.length > 0 && !selectedTable) {
      setSelectedTable(tables[0]);
    }
  }, [tables, selectedTable]);

  return (
    <div className="flex flex-col flex-1 overflow-hidden">
      {/* Top bar */}
      <div className="flex items-center gap-4 px-4 py-2 border-b shrink-0">
        <Select
          value={selectedSchema}
          onValueChange={(value) => {
            setSelectedSchema(value);
            setSelectedTable(""); // a new schema's tables differ; pick afresh
          }}
        >
          <SelectTrigger className="w-44">
            <SelectValue placeholder="Select a schema" />
          </SelectTrigger>
          <SelectContent>
            {schemasLoading ? (
              <SelectItem value="loading" disabled>
                Loading...
              </SelectItem>
            ) : (
              schemas?.map((s) => (
                <SelectItem key={s} value={s}>
                  {s}
                </SelectItem>
              ))
            )}
          </SelectContent>
        </Select>

        <Select value={selectedTable} onValueChange={setSelectedTable}>
          <SelectTrigger className="w-70">
            <SelectValue placeholder="Select a table" />
          </SelectTrigger>
          <SelectContent>
            {tablesLoading ? (
              <SelectItem value="loading" disabled>
                Loading...
              </SelectItem>
            ) : (
              tables?.map((table) => (
                <SelectItem key={table} value={table}>
                  {formatTableName(table)}
                </SelectItem>
              ))
            )}
          </SelectContent>
        </Select>

        {/* Subtle refresh cue while a table switch reloads metadata in place. */}
        <FetchingIndicator when={metaFetching && !!tableMeta} />
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-4 space-y-6">
        <DataLoading
          isPending={tableMetaFirstLoad}
          error={metaError}
          onRetry={() => refetchMeta()}
          variant="page"
        />

        {tableMeta && (
          <>
            {/* Overview */}
            <OverviewCards meta={tableMeta} />

            {/* Columns */}
            <div className="space-y-2">
              <SectionHeader title="Columns" />
              <ColumnsTable
                columns={tableMeta.columns}
                meta={tableMeta}
                onColumnClick={(column) =>
                  navigate({
                    to: "/column-analysis",
                    search: {
                      schema: selectedSchema,
                      table: selectedTable,
                      column,
                    },
                  })
                }
              />
            </div>

            {/* Row Counts Over Time */}
            <div className="space-y-2">
              <SectionHeader title="Row Counts Over Time" />
              <div className="rounded-lg border bg-card p-4">
                <RowCountsChart
                  selectedSchema={selectedSchema}
                  selectedTable={selectedTable}
                />
              </div>
            </div>

            {/* Storage */}
            {tableMeta.redshift_storage && (
              <div className="space-y-2">
                <SectionHeader title="Storage" />
                <RedshiftStorageSection storage={tableMeta.redshift_storage} />
              </div>
            )}

            {/* Storage (Snowflake-backed tables) — same heading as the Redshift
                block above so the section title never reveals the backend. */}
            {tableMeta.snowflake_details && (
              <div className="space-y-2">
                <SectionHeader title="Storage" />
                <SnowflakeDetailsSection details={tableMeta.snowflake_details} />
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
