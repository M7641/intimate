"use no memo";

import React from "react";
import { useQueries } from "@tanstack/react-query";
import { useSearch } from "@tanstack/react-router";

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
} from "chart.js";
import { Bar } from "react-chartjs-2";
import { Plus, X } from "lucide-react";

import DataLoading from "@/components/DataLoad";
import NoDataAvailable from "@/components/NoDataAvailable";
import StatCard from "@/components/StatCard";
import FetchingIndicator from "@/components/FetchingIndicator";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  formatNumber,
  formatTimestampShort,
  formatTableName,
  formatColumnName,
} from "@/lib/format";

import {
  useDataExplorerSchemas,
  useDataExplorerTables,
  useDataExplorerColumns,
  useDataExplorerTimestamps,
  useColumnStats,
  useValueDistribution,
  columnStatsQueryOptions,
  valueDistributionQueryOptions,
} from "@/pages/_shared/api";
import type { ColumnStats, ValueDistribution } from "@/pages/_shared/types";
import { useTheme } from "@/components/themeProvider";
import { readChartPalette } from "@/lib/chartColors";

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

// CSS-var strings for inline DOM styles (compare pills). The DOM resolves
// `var(...)` natively, so these stay as variables; canvas charts instead use
// the resolved palette from `readChartPalette` (see usePalette below).
const CHART_COLORS = ["--chart-1", "--chart-2", "--chart-3", "--chart-4"].map(
  (v) => `var(${v})`,
);

/** Theme-reactive resolved chart palette for canvas (Chart.js). */
function usePalette() {
  const { theme } = useTheme();
  return React.useMemo(() => readChartPalette(), [theme]);
}

// --- Stats Grid ---

function StatsGrid({ stats }: { stats: ColumnStats }) {
  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
        <StatCard label="Total Rows" value={formatNumber(stats.total_count)} />
        <StatCard
          label="Nulls"
          value={formatNumber(stats.null_count)}
          sub={`${formatNumber(stats.null_pct)}%`}
          hint="Rows with no value for this column. A high share can signal a data quality gap."
        />
        <StatCard
          label="Distinct"
          value={formatNumber(stats.distinct_count)}
          sub={`${formatNumber(stats.cardinality_pct)}%`}
          hint="Number of unique values, and what share of all rows that represents."
        />
        {stats.is_numeric && (
          <StatCard
            label="Range"
            value={`${formatNumber(stats.min_val)} to ${formatNumber(stats.max_val)}`}
            hint="Lowest and highest values in this column."
          />
        )}
      </div>

      {stats.is_numeric && (
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          <StatCard
            label="Mean"
            value={formatNumber(stats.mean_val)}
            hint="The average value."
          />
          <StatCard
            label="Median"
            value={formatNumber(stats.median_val)}
            hint="The middle value: half the rows are below it, half above."
          />
          <StatCard
            label="Spread (Std Dev)"
            value={formatNumber(stats.stddev_val)}
            hint="How far values typically sit from the average. Larger means more variable."
          />
          <StatCard
            label="Middle 50%"
            value={`${formatNumber(stats.p25)} to ${formatNumber(stats.p75)}`}
            hint="The range covering the middle half of values (25th to 75th percentile)."
          />
        </div>
      )}
    </div>
  );
}

// --- Compare Table ---

function CompareTable({
  timestamps,
  statsResults,
}: {
  timestamps: string[];
  statsResults: (ColumnStats | undefined)[];
}) {
  const isNumeric = statsResults.some((s) => s?.is_numeric);

  const rows: { label: string; key: (s: ColumnStats) => string }[] = [
    { label: "Total", key: (s) => formatNumber(s.total_count) },
    {
      label: "Nulls",
      key: (s) =>
        `${formatNumber(s.null_count)} (${formatNumber(s.null_pct)}%)`,
    },
    {
      label: "Distinct",
      key: (s) =>
        `${formatNumber(s.distinct_count)} (${formatNumber(s.cardinality_pct)}%)`,
    },
  ];

  if (isNumeric) {
    rows.push(
      { label: "Min", key: (s) => formatNumber(s.min_val) },
      { label: "Max", key: (s) => formatNumber(s.max_val) },
      { label: "Mean", key: (s) => formatNumber(s.mean_val) },
      { label: "Median", key: (s) => formatNumber(s.median_val) },
      { label: "Std Dev", key: (s) => formatNumber(s.stddev_val) },
      { label: "P25", key: (s) => formatNumber(s.p25) },
      { label: "P75", key: (s) => formatNumber(s.p75) },
    );
  }

  return (
    <div className="rounded-lg border overflow-hidden">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b bg-muted/50">
            <th className="text-left px-4 py-2 font-medium text-muted-foreground">
              Metric
            </th>
            {timestamps.map((ts, i) => (
              <th
                key={ts}
                className="text-right px-4 py-2 font-medium"
                style={{ color: CHART_COLORS[i] }}
              >
                {formatTimestampShort(ts)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.label} className="border-b last:border-b-0">
              <td className="px-4 py-2 text-muted-foreground">{row.label}</td>
              {statsResults.map((s, i) => (
                <td
                  key={timestamps[i]}
                  className="text-right px-4 py-2 tabular-nums"
                >
                  {s ? row.key(s) : "—"}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// --- Distribution Chart ---

function DistributionChart({
  distributions,
  timestamps,
  isComparing,
}: {
  distributions: (ValueDistribution | undefined)[];
  timestamps: string[];
  isComparing: boolean;
}) {
  const primary = distributions[0];
  if (!primary || primary.distribution.length === 0) {
    return (
      <NoDataAvailable
        message="No distribution to show"
        description="There aren't enough values to chart a distribution for this column."
      />
    );
  }

  if (primary.is_numeric) {
    return (
      <NumericChart
        distributions={distributions}
        timestamps={timestamps}
        isComparing={isComparing}
      />
    );
  }

  return (
    <CategoricalChart
      distributions={distributions}
      timestamps={timestamps}
      isComparing={isComparing}
    />
  );
}

function NumericChart({
  distributions,
  timestamps,
  isComparing,
}: {
  distributions: (ValueDistribution | undefined)[];
  timestamps: string[];
  isComparing: boolean;
}) {
  const palette = usePalette();

  const chartData = React.useMemo(() => {
    const primary = distributions[0];
    if (!primary) return { labels: [], datasets: [] };

    const labels = primary.distribution.map((item) =>
      item.bin_min != null && item.bin_max != null
        ? `${formatNumber(item.bin_min)}–${formatNumber(item.bin_max)}`
        : `Bin ${item.bucket}`,
    );

    if (isComparing) {
      const series = palette.series;
      const datasets = distributions.map((dist, i) => ({
        label: formatTimestampShort(timestamps[i]),
        data: primary.distribution.map(
          (_, idx) => dist?.distribution[idx]?.count ?? 0,
        ),
        backgroundColor: series[i % series.length],
        borderRadius: 3,
      }));
      return { labels, datasets };
    }

    return {
      labels,
      datasets: [
        {
          label: "count",
          data: primary.distribution.map((item) => item.count),
          backgroundColor: palette.series[0],
          borderRadius: 4,
          maxBarThickness: 48,
        },
      ],
    };
  }, [distributions, timestamps, isComparing, palette]);

  const options = React.useMemo(
    () => ({
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: {
          display: isComparing,
          labels: { color: palette.ticks, font: { size: 12 } },
        },
        tooltip: {},
      },
      scales: {
        x: {
          ticks: {
            color: palette.ticks,
            font: { size: 11 },
            maxRotation: 45,
            minRotation: 45,
          },
          grid: { display: false },
          border: { color: palette.grid },
        },
        y: {
          ticks: {
            color: palette.ticks,
            font: { size: 12 },
            callback: (v: string | number) => Number(v).toLocaleString(),
          },
          grid: { color: palette.grid },
          border: { display: false },
        },
      },
    }),
    [isComparing, palette],
  );

  return (
    <div style={{ height: "360px" }}>
      <Bar data={chartData} options={options} />
    </div>
  );
}

function CategoricalChart({
  distributions,
  timestamps,
  isComparing,
}: {
  distributions: (ValueDistribution | undefined)[];
  timestamps: string[];
  isComparing: boolean;
}) {
  const palette = usePalette();

  const chartData = React.useMemo(() => {
    const primary = distributions[0];
    if (!primary) return { labels: [], datasets: [] };

    const labels = primary.distribution.map((item) => item.value ?? "(null)");

    if (isComparing) {
      const series = palette.series;
      const datasets = distributions.map((dist, i) => ({
        label: formatTimestampShort(timestamps[i]),
        data: primary.distribution.map(
          (item) =>
            dist?.distribution.find((d) => d.value === item.value)?.count ?? 0,
        ),
        backgroundColor: series[i % series.length],
        borderRadius: 3,
      }));
      return { labels, datasets };
    }

    return {
      labels,
      datasets: [
        {
          label: "count",
          data: primary.distribution.map((item) => item.count),
          backgroundColor: palette.series[0],
          borderRadius: 4,
        },
      ],
    };
  }, [distributions, timestamps, isComparing, palette]);

  const barHeight = React.useMemo(() => {
    const primary = distributions[0];
    const count = primary ? primary.distribution.length : 0;
    return Math.max(360, count * 32);
  }, [distributions]);

  const options = React.useMemo(
    () => ({
      indexAxis: "y" as const,
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: {
          display: isComparing,
          labels: { color: palette.ticks, font: { size: 12 } },
        },
        tooltip: {},
      },
      scales: {
        x: {
          ticks: {
            color: palette.ticks,
            font: { size: 12 },
            callback: (v: string | number) => Number(v).toLocaleString(),
          },
          grid: { color: palette.grid },
          border: { display: false },
        },
        y: {
          ticks: { color: palette.ticks, font: { size: 11 } },
          grid: { display: false },
          border: { color: palette.grid },
        },
      },
    }),
    [isComparing, palette],
  );

  return (
    <div style={{ height: `${barHeight}px` }}>
      <Bar data={chartData} options={options} />
    </div>
  );
}

// --- Main Page ---

export default function ColumnAnalysisPage() {
  // Deep link from Table Info's column rows: ?schema=&table=&column=
  const search = useSearch({ strict: false }) as {
    schema?: string;
    table?: string;
    column?: string;
  };
  // Seed from the URL first (a deep link wins), then the remembered choice.
  const [selectedSchema, setSelectedSchema] = React.useState(
    () => search.schema ?? localStorage.getItem(SCHEMA_STORAGE_KEY) ?? "",
  );
  const [selectedTable, setSelectedTable] = React.useState(search.table ?? "");
  const [selectedColumn, setSelectedColumn] = React.useState(
    search.column ?? "",
  );
  const [selectedTimestamp, setSelectedTimestamp] = React.useState("");
  const [compareTimestamps, setCompareTimestamps] = React.useState<string[]>(
    [],
  );

  // Follow deep-link changes after mount: useState only seeds the selection
  // once, so arriving from a different Table Info column while this page is
  // still mounted wouldn't otherwise move the selects.
  React.useEffect(() => {
    if (search.schema && search.schema !== selectedSchema)
      setSelectedSchema(search.schema);
    if (search.table && search.table !== selectedTable)
      setSelectedTable(search.table);
    if (search.column && search.column !== selectedColumn)
      setSelectedColumn(search.column);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [search.schema, search.table, search.column]);

  const { data: schemas, isPending: schemasLoading } = useDataExplorerSchemas();

  // Remember the schema across visits.
  React.useEffect(() => {
    if (selectedSchema) localStorage.setItem(SCHEMA_STORAGE_KEY, selectedSchema);
  }, [selectedSchema]);

  // Auto-select schema: prefer "stage", else first available. Also corrects a
  // remembered schema that no longer exists in the available set.
  React.useEffect(() => {
    if (schemas && schemas.length > 0) {
      if (!selectedSchema || !schemas.includes(selectedSchema)) {
        setSelectedSchema(schemas.includes("stage") ? "stage" : schemas[0]);
      }
    }
  }, [schemas, selectedSchema]);

  const { data: tables, isPending: tablesLoading } =
    useDataExplorerTables(selectedSchema);
  const { data: columns, isPending: columnsLoading } =
    useDataExplorerColumns(selectedSchema, selectedTable);
  const { data: timestamps, isPending: timestampsLoading } =
    useDataExplorerTimestamps(selectedSchema, selectedTable);

  // Auto-select first table
  React.useEffect(() => {
    if (tables && tables.length > 0 && !selectedTable) {
      setSelectedTable(tables[0]);
    }
  }, [tables, selectedTable]);

  // Auto-select first column when columns load — but keep a deep-linked column
  // (?column=) if it exists in the set, so navigating to a specific column lands
  // on it instead of snapping back to the first.
  React.useEffect(() => {
    if (columns && columns.length > 0) {
      const names = columns.map((c) => c.column_name);
      if (!selectedColumn || !names.includes(selectedColumn)) {
        setSelectedColumn(columns[0].column_name);
      }
    }
    // selectedColumn is read for the guard but intentionally not a dependency:
    // we only re-evaluate when the column set itself changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [columns]);

  // Auto-select latest timestamp
  React.useEffect(() => {
    if (timestamps && timestamps.length > 0) {
      setSelectedTimestamp(timestamps[0].load_timestamp);
    }
  }, [timestamps]);

  // Reset everything when schema changes
  const handleSchemaChange = (schema: string) => {
    setSelectedSchema(schema);
    setSelectedTable("");
    setSelectedColumn("");
    setSelectedTimestamp("");
    setCompareTimestamps([]);
  };

  // Reset column + timestamp when table changes
  const handleTableChange = (table: string) => {
    setSelectedTable(table);
    setSelectedColumn("");
    setSelectedTimestamp("");
    setCompareTimestamps([]);
  };

  // All timestamps for queries (primary + compare)
  const allTimestamps = [selectedTimestamp, ...compareTimestamps].filter(
    Boolean,
  );
  const isComparing = compareTimestamps.length > 0;

  // Primary data hooks
  const {
    data: primaryStats,
    error: statsError,
    isFetching: statsFetching,
    refetch: refetchStats,
  } = useColumnStats(
    selectedSchema,
    selectedTable,
    selectedColumn,
    selectedTimestamp,
  );

  const {
    data: primaryDist,
    error: distError,
    isFetching: distFetching,
  } = useValueDistribution(
    selectedSchema,
    selectedTable,
    selectedColumn,
    selectedTimestamp,
  );

  // Compare data — one query per *real* compare timestamp. useQueries lets us
  // map over the dynamic list, and (unlike fixed slots) never fires a redundant
  // query for an empty slot.
  const compareStats = useQueries({
    queries: compareTimestamps.map((ts) =>
      columnStatsQueryOptions(
        selectedSchema,
        selectedTable,
        selectedColumn,
        ts,
      ),
    ),
  });
  const compareDist = useQueries({
    queries: compareTimestamps.map((ts) =>
      valueDistributionQueryOptions(
        selectedSchema,
        selectedTable,
        selectedColumn,
        ts,
      ),
    ),
  });

  // Always keep the primary result, plus one slot per compare timestamp.
  // (Basing this on allTimestamps would drop the primary on tables that have
  // no load_timestamp, where selectedTimestamp is "" — hiding the chart.)
  const resultCount = compareTimestamps.length + 1;

  const allStatsResults: (ColumnStats | undefined)[] = [
    primaryStats,
    ...compareStats.map((q) => q.data),
  ].slice(0, resultCount);

  const allDistResults: (ValueDistribution | undefined)[] = [
    primaryDist,
    ...compareDist.map((q) => q.data),
  ].slice(0, resultCount);

  const addCompareTimestamp = () => {
    if (!timestamps || compareTimestamps.length >= 3) return;
    const used = new Set([selectedTimestamp, ...compareTimestamps]);
    const next = timestamps.find((t) => !used.has(t.load_timestamp));
    if (next) {
      setCompareTimestamps((prev) => [...prev, next.load_timestamp]);
    }
  };

  const removeCompareTimestamp = (idx: number) => {
    setCompareTimestamps((prev) => prev.filter((_, i) => i !== idx));
  };

  const availableForCompare =
    timestamps?.filter(
      (t) =>
        t.load_timestamp !== selectedTimestamp &&
        !compareTimestamps.includes(t.load_timestamp),
    ) ?? [];

  const canAddCompare =
    compareTimestamps.length < 3 && availableForCompare.length > 0;

  // First-load skeleton = nothing to show yet (no data, no error). On a
  // column/table/timestamp switch, keepPreviousData keeps the prior result on
  // screen so reading `.data` never suspends and re-mounts the page.
  const isLoading =
    (!primaryStats && !statsError) || (!primaryDist && !distError);

  // Subtle refresh cue while stale data stays on screen during an in-place
  // refetch (the keepPreviousData case — distinct from the first-load skeleton).
  const isRefreshing =
    (statsFetching && !!primaryStats) || (distFetching && !!primaryDist);

  return (
    <div className="flex flex-col flex-1 overflow-hidden">
      {/* Top bar */}
      <div className="flex items-center gap-3 px-4 py-2 border-b shrink-0 flex-wrap">
        {/* Schema selector */}
        <Select value={selectedSchema} onValueChange={handleSchemaChange}>
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

        {/* Table selector */}
        <Select value={selectedTable} onValueChange={handleTableChange}>
          <SelectTrigger className="w-56">
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

        {/* Column selector */}
        <Select value={selectedColumn} onValueChange={setSelectedColumn}>
          <SelectTrigger className="w-48">
            <SelectValue placeholder="Select a column" />
          </SelectTrigger>
          <SelectContent>
            {columnsLoading ? (
              <SelectItem value="loading" disabled>
                Loading...
              </SelectItem>
            ) : (
              columns?.map((col) => (
                <SelectItem key={col.column_name} value={col.column_name}>
                  <span>{formatColumnName(col.column_name)}</span>
                  <span className="ml-2 text-xs text-muted-foreground">
                    {col.data_type}
                  </span>
                </SelectItem>
              ))
            )}
          </SelectContent>
        </Select>

        {/* Timestamp selector — hidden when table has no load_timestamp */}
        {!timestampsLoading && timestamps && timestamps.length > 0 && (
          <>
            <Select
              value={selectedTimestamp}
              onValueChange={setSelectedTimestamp}
            >
              <SelectTrigger className="w-52">
                <SelectValue placeholder="Select timestamp" />
              </SelectTrigger>
              <SelectContent>
                {timestamps.map((ts) => (
                  <SelectItem key={ts.load_timestamp} value={ts.load_timestamp}>
                    {formatTimestampShort(ts.load_timestamp)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            {/* Latest button */}
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                if (timestamps && timestamps.length > 0) {
                  setSelectedTimestamp(timestamps[0].load_timestamp);
                }
              }}
              disabled={!timestamps?.length}
            >
              Latest
            </Button>

            {/* Compare button */}
            <Button
              variant="outline"
              size="sm"
              onClick={addCompareTimestamp}
              disabled={!canAddCompare}
            >
              <Plus className="size-3.5" />
              Compare
            </Button>
          </>
        )}

        {/* Refresh cue while a switch reloads stats/distribution in place. */}
        <FetchingIndicator when={isRefreshing} />

        {/* Column type badge */}
        {primaryStats && (
          <span className="ml-auto text-xs text-muted-foreground">
            {primaryStats.data_type}
            {primaryStats.is_numeric ? " · numeric" : " · categorical"}
          </span>
        )}
      </div>

      {/* Compare bar — only present while comparing, so the primary selectors
          above stay uncluttered. */}
      {compareTimestamps.length > 0 && (
        <div className="flex items-center gap-2 px-4 py-2 border-b shrink-0 flex-wrap bg-muted/30">
          <span className="text-xs font-medium text-muted-foreground mr-1">
            Comparing to
          </span>
          {compareTimestamps.map((ts, idx) => (
            <div
              key={ts}
              className="flex items-center gap-1.5 rounded-md border bg-background px-2.5 py-1 text-xs"
              style={{ borderColor: CHART_COLORS[idx + 1] }}
            >
              <div
                className="size-2 rounded-full"
                style={{ backgroundColor: CHART_COLORS[idx + 1] }}
              />
              <Select
                value={ts}
                onValueChange={(newTs) => {
                  setCompareTimestamps((prev) =>
                    prev.map((t, i) => (i === idx ? newTs : t)),
                  );
                }}
              >
                <SelectTrigger className="border-0 shadow-none h-auto p-0 text-xs w-36">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {timestamps
                    ?.filter(
                      (t) =>
                        t.load_timestamp === ts ||
                        (t.load_timestamp !== selectedTimestamp &&
                          !compareTimestamps.includes(t.load_timestamp)),
                    )
                    .map((t) => (
                      <SelectItem
                        key={t.load_timestamp}
                        value={t.load_timestamp}
                      >
                        {formatTimestampShort(t.load_timestamp)}
                      </SelectItem>
                    ))}
                </SelectContent>
              </Select>
              <button
                onClick={() => removeCompareTimestamp(idx)}
                className="text-muted-foreground hover:text-foreground"
              >
                <X className="size-3" />
              </button>
            </div>
          ))}
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-auto p-4 space-y-4">
        <DataLoading
          isPending={isLoading}
          error={statsError}
          onRetry={() => refetchStats()}
          variant="cards"
        />
        {/* Stats load as cards above; mirror the distribution panel below with a
            chart skeleton so the whole view fills in while data arrives. */}
        {isLoading && !statsError && (
          <DataLoading isPending={true} variant="chart" />
        )}

        {!isLoading && !statsError && !primaryStats && selectedColumn && (
          <NoDataAvailable
            message="No statistics for this column"
            description="This column has no values at the selected load time. Try another column or load time."
          />
        )}

        {!isLoading && !statsError && primaryStats && (
          <>
            {/* Stats section */}
            {isComparing ? (
              <CompareTable
                timestamps={allTimestamps}
                statsResults={allStatsResults}
              />
            ) : (
              <StatsGrid stats={primaryStats} />
            )}

            {/* Distribution chart */}
            <div className="rounded-lg border bg-card p-4">
              <h3 className="text-sm font-medium text-muted-foreground mb-3">
                Distribution
              </h3>
              <DistributionChart
                distributions={allDistResults}
                timestamps={allTimestamps}
                isComparing={isComparing}
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
