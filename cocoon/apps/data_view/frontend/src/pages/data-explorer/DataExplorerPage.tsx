import React from "react";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { FaDownload } from "react-icons/fa";

import MTable from "@/components/MTable";
import DataLoading from "@/components/DataLoad";
import NoDataAvailable from "@/components/NoDataAvailable";
import { cn } from "@/lib/utils";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import FilterSidebar from "./components/FilterSidebar";
import {
  formatTimestamp,
  formatTableName,
  formatColumnName,
} from "@/lib/format";

import type {
  SnapshotData,
  ColumnFilter,
  ColumnMeta,
} from "@/pages/_shared/types";
import {
  useDataExplorerSchemas,
  useDataExplorerTables,
  useDataExplorerTimestamps,
  useDataExplorerColumns,
  useDataExplorerData,
} from "@/pages/_shared/api";

/** Server-side row cap for an unfiltered preview; single source of truth. */
const ROW_CAP = 1000;

/** Upper bound for a CSV export, so a stray large input can't hang the browser. */
const MAX_EXPORT = 100_000;

type ExplorerSearch = { schema?: string; table?: string; ts?: string };

export default function DataExplorerPage() {
  // The current selection lives in the URL (?schema=&table=&ts=) so it survives
  // a refresh and can be deep-linked from the home page. We seed state from the
  // URL once on mount, then mirror later picks back into it below.
  const search = useSearch({ strict: false }) as ExplorerSearch;
  const navigate = useNavigate();

  // User intent. These hold what the user explicitly picked; a pick may not be
  // valid against the data that has loaded yet, so the queries below run off
  // the *effective* values derived from these instead of the raw state.
  const [selectedSchema, setSelectedSchema] = React.useState<string>(
    search.schema ?? "",
  );
  const [selectedTable, setSelectedTable] = React.useState<string>(
    search.table ?? "",
  );
  const [selectedTimestamp, setSelectedTimestamp] = React.useState<string>(
    search.ts ?? "",
  );
  const [filters, setFilters] = React.useState<ColumnFilter[]>([]);
  const [downloadLimit, setDownloadLimit] = React.useState<string>(
    String(ROW_CAP),
  );
  const [isDownloading, setIsDownloading] = React.useState(false);
  const [downloadOpen, setDownloadOpen] = React.useState(false);

  // Write the selection back into the URL on user picks only — never from a
  // reactive effect, which would let `navigate` re-enter and loop. `replace`
  // keeps history clean; the updater merges so we touch only the given keys.
  const persist = (patch: ExplorerSearch) =>
    navigate({
      to: ".",
      replace: true,
      search: (prev: ExplorerSearch) => ({ ...prev, ...patch }),
    });

  // ── Effective selection ────────────────────────────────────────────────
  // Each level falls back to a sensible default until the user's pick is valid
  // for the loaded options. Deriving the selection (instead of syncing it with
  // effects) keeps the whole chain coherent in one render pass: no transient
  // empty states, no reset cascade, so nothing tears down between switches.
  const schemas = useDataExplorerSchemas();
  const effectiveSchema = React.useMemo(() => {
    const list = schemas.data ?? [];
    if (list.length === 0) return "";
    if (selectedSchema && list.includes(selectedSchema)) return selectedSchema;
    return list.includes("stage") ? "stage" : list[0];
  }, [schemas.data, selectedSchema]);

  const tables = useDataExplorerTables(effectiveSchema);
  const effectiveTable = React.useMemo(() => {
    const list = tables.data ?? [];
    if (list.length === 0) return "";
    return selectedTable && list.includes(selectedTable)
      ? selectedTable
      : list[0];
  }, [tables.data, selectedTable]);

  const timestamps = useDataExplorerTimestamps(effectiveSchema, effectiveTable);
  const columnsInfo = useDataExplorerColumns(effectiveSchema, effectiveTable);
  const effectiveTimestamp = React.useMemo(() => {
    const list = timestamps.data ?? [];
    if (list.length === 0) return "";
    return selectedTimestamp &&
      list.some((t) => t.load_timestamp === selectedTimestamp)
      ? selectedTimestamp
      : list[0].load_timestamp;
  }, [timestamps.data, selectedTimestamp]);

  // Filters belong to a (table, load) pair; clear them when that context
  // changes. This is the one genuine side effect left in the selection flow.
  React.useEffect(() => {
    setFilters([]);
  }, [effectiveTable, effectiveTimestamp]);

  const dataQuery = useDataExplorerData(
    effectiveSchema,
    effectiveTable,
    effectiveTimestamp,
    filters,
  );

  const columns = React.useMemo<ColumnDef<SnapshotData, unknown>[]>(() => {
    const snapshotData = dataQuery.data;
    if (!snapshotData || snapshotData.length === 0) return [];

    const columnHelper = createColumnHelper<SnapshotData>();
    const keys = Object.keys(snapshotData[0]);

    return keys.map((key) => {
      const sample = snapshotData.find((row) => row[key] != null)?.[key];
      const isNumeric = typeof sample === "number";
      return columnHelper.accessor((row) => row[key], {
        id: key,
        header: formatColumnName(key),
        cell: (info) => {
          const value = info.getValue();
          if (value === null || value === undefined) return "-";
          if (typeof value === "number") {
            return Number.isInteger(value)
              ? value.toLocaleString()
              : value.toFixed(2);
          }
          return String(value);
        },
        size: 120,
        meta: isNumeric ? { align: "right" as const } : undefined,
      });
    }) as ColumnDef<SnapshotData, unknown>[];
  }, [dataQuery.data]);

  // Compute column metadata for filtering.
  // Uses columnsInfo from API for column names, computes unique values from data.
  const columnMeta = React.useMemo<ColumnMeta[]>(() => {
    const snapshotData = dataQuery.data;
    const cols = columnsInfo.data;

    // Use columns from API endpoint if available
    if (cols && cols.length > 0) {
      return cols.map((col) => {
        // Compute unique values from data if available
        const uniqueValues = snapshotData
          ? [
              ...new Set(
                snapshotData.map((row) => String(row[col.column_name] ?? "")),
              ),
            ].sort()
          : [];
        return {
          key: col.column_name,
          displayName: formatColumnName(col.column_name),
          uniqueValues,
          cardinality: uniqueValues.length,
        };
      });
    }

    // Fallback: derive columns from data if columnsInfo not yet loaded
    if (!snapshotData || snapshotData.length === 0) return [];

    const keys = Object.keys(snapshotData[0]);
    return keys.map((key) => {
      const uniqueValues = [
        ...new Set(snapshotData.map((row) => String(row[key] ?? ""))),
      ].sort();
      return {
        key,
        displayName: formatColumnName(key),
        uniqueValues,
        cardinality: uniqueValues.length,
      };
    });
  }, [columnsInfo.data, dataQuery.data]);

  // Check if any filters are active (for UI feedback)
  const hasActiveFilters = filters.some((f) => f.column && f.value);

  // The very first load has no previous data to keep — that alone warrants the
  // full skeleton. Every later switch keeps the old rows and just dims them.
  const firstLoad = dataQuery.isPending && !dataQuery.data;
  // Any in-flight work that should dim the current view and show the top bar.
  const loading =
    dataQuery.isFetching ||
    dataQuery.isPlaceholderData ||
    timestamps.isFetching;

  const handleDownload = async () => {
    const parsed = parseInt(downloadLimit, 10) || ROW_CAP;
    const limit = Math.min(MAX_EXPORT, Math.max(1, parsed));
    setIsDownloading(true);
    try {
      const params = new URLSearchParams({
        schema: effectiveSchema,
        limit: String(limit),
      });
      if (effectiveTimestamp) {
        params.append("load_timestamp", effectiveTimestamp);
      }
      const activeFilters = filters.filter((f) => f.column && f.value);
      if (activeFilters.length > 0) {
        params.append("filters", JSON.stringify(activeFilters));
      }
      const response = await fetch(
        `/api/data_view/data/${effectiveTable}?${params}`,
      );
      if (!response.ok) throw new Error("Download failed");
      const rows: SnapshotData[] = await response.json();
      if (rows.length === 0) return;

      const keys = Object.keys(rows[0]);
      const headerRow = keys.join(",");
      const csvRows = rows.map((row) =>
        keys
          .map((key) => {
            const value = row[key];
            if (value === null || value === undefined) return "";
            const str = String(value);
            if (str.includes(",") || str.includes('"') || str.includes("\n")) {
              return `"${str.replace(/"/g, '""')}"`;
            }
            return str;
          })
          .join(","),
      );
      const csvContent = [headerRow, ...csvRows].join("\n");
      const blob = new Blob([csvContent], { type: "text/csv;charset=utf-8;" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `${effectiveTable}_${new Date().toISOString().split("T")[0]}.csv`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);
    } finally {
      setIsDownloading(false);
      setDownloadOpen(false);
    }
  };

  const showTimestampControl =
    !!effectiveTable &&
    (timestamps.isPending || (timestamps.data?.length ?? 0) > 0);

  return (
    <div className="flex flex-col flex-1 overflow-hidden">
      {/* Top bar: schema/table/timestamp selectors + row count */}
      <div className="flex items-center gap-4 px-4 py-2 border-b shrink-0">
        <Select
          value={effectiveSchema}
          onValueChange={(v) => {
            if (!v) return;
            setSelectedSchema(v);
            // Table and load belong to the old schema; drop them so the URL
            // doesn't carry a stale pair, and let them re-derive.
            setSelectedTable("");
            setSelectedTimestamp("");
            persist({ schema: v, table: undefined, ts: undefined });
          }}
          disabled={schemas.isPending}
        >
          <SelectTrigger className="w-44">
            <SelectValue placeholder="Select a schema" />
          </SelectTrigger>
          <SelectContent>
            {(schemas.data ?? []).map((s) => (
              <SelectItem key={s} value={s}>
                {s}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Select
          value={effectiveTable}
          onValueChange={(v) => {
            if (!v) return;
            setSelectedTable(v);
            // A new table resets the load time; let it re-derive to latest.
            setSelectedTimestamp("");
            persist({ table: v, ts: undefined });
          }}
          disabled={tables.isPending}
        >
          <SelectTrigger className="w-70">
            <SelectValue placeholder="Select a table" />
          </SelectTrigger>
          <SelectContent>
            {(tables.data ?? []).map((table) => (
              <SelectItem key={table} value={table}>
                {formatTableName(table)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        {/* Keep the timestamp control mounted across table switches — disabled
            while its list loads — so the toolbar never reflows. It only hides
            when a table is selected and genuinely has no timestamps. */}
        {showTimestampControl && (
          <>
            <Select
              value={effectiveTimestamp}
              onValueChange={(v) => {
                if (!v) return;
                setSelectedTimestamp(v);
                persist({ ts: v });
              }}
              disabled={timestamps.isPending || !timestamps.data?.length}
            >
              <SelectTrigger className="w-70">
                <SelectValue
                  placeholder={
                    timestamps.isPending ? "Loading..." : "Select a timestamp"
                  }
                />
              </SelectTrigger>
              <SelectContent>
                {(timestamps.data ?? []).map((ts) => (
                  <SelectItem
                    key={ts.load_timestamp}
                    value={ts.load_timestamp}
                  >
                    {formatTimestamp(ts.load_timestamp)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            <Button
              variant="outline"
              onClick={() => {
                const list = timestamps.data;
                if (list && list.length > 0) {
                  setSelectedTimestamp(list[0].load_timestamp);
                  persist({ ts: list[0].load_timestamp });
                }
              }}
              disabled={!timestamps.data?.length}
            >
              Latest
            </Button>
          </>
        )}

        <Popover open={downloadOpen} onOpenChange={setDownloadOpen}>
          <PopoverTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              disabled={!effectiveTable || dataQuery.isPending}
              className="ml-auto"
            >
              <FaDownload className="mr-1.5 h-3 w-3" />
              Download
            </Button>
          </PopoverTrigger>
          <PopoverContent align="end" className="w-64">
            <div className="flex flex-col gap-3">
              <label className="text-sm font-medium">Row limit</label>
              <Input
                type="number"
                min={1}
                max={MAX_EXPORT}
                value={downloadLimit}
                onChange={(e) => setDownloadLimit(e.target.value)}
                placeholder={String(ROW_CAP)}
              />
              <p className="text-xs text-muted-foreground">
                Up to {MAX_EXPORT.toLocaleString()} rows per export.
              </p>
              <Button
                size="sm"
                onClick={handleDownload}
                disabled={isDownloading}
              >
                {isDownloading ? "Downloading..." : "Export CSV"}
              </Button>
            </div>
          </PopoverContent>
        </Popover>

        <span className="text-sm text-muted-foreground">
          {loading ? (
            "Loading..."
          ) : (
            <>
              Showing {(dataQuery.data?.length ?? 0).toLocaleString()} rows
              {hasActiveFilters && " (filtered)"}
              {!hasActiveFilters && (
                <span
                  title={`Results are capped at ${ROW_CAP.toLocaleString()} rows. Use filters to narrow down.`}
                >
                  {" "}
                  (limited to {ROW_CAP.toLocaleString()})
                </span>
              )}
            </>
          )}
        </span>
      </div>

      {/* Main area: sidebar + content */}
      <div className="flex flex-1 overflow-hidden">
        <FilterSidebar
          columns={columnMeta}
          filters={filters}
          onFiltersChange={setFilters}
          disabled={columnsInfo.isPending || !columnsInfo.data?.length}
          selectedSchema={effectiveSchema}
          selectedTable={effectiveTable}
          selectedTimestamp={effectiveTimestamp}
        />

        <div className="relative flex flex-1 flex-col overflow-hidden p-4 min-h-0">
          {/* Slim top progress bar while any switch is in flight. */}
          {loading && (
            <div
              aria-hidden="true"
              className="pointer-events-none absolute inset-x-0 top-0 z-10 h-0.5 overflow-hidden"
            >
              <div className="h-full w-full animate-pulse bg-primary/70" />
            </div>
          )}

          {/* First load (or error) gets the full skeleton — nothing to keep. */}
          <DataLoading
            isPending={firstLoad}
            error={dataQuery.error}
            onRetry={() => dataQuery.refetch()}
          />

          {/* Data present (current or kept from the previous table): render the
              table, dimmed while the next set loads so it never tears down. */}
          {!dataQuery.isError &&
            dataQuery.data &&
            dataQuery.data.length > 0 && (
              <div
                className={cn(
                  "flex min-h-0 flex-1 flex-col transition-opacity duration-200",
                  loading && "pointer-events-none opacity-50",
                )}
              >
                <MTable
                  data={dataQuery.data}
                  columns={columns}
                  searchable
                  fill
                  columnShapes
                />
              </div>
            )}

          {/* Settled and genuinely empty. */}
          {!dataQuery.isError &&
            !firstLoad &&
            !loading &&
            (dataQuery.data?.length ?? 0) === 0 &&
            (hasActiveFilters ? (
              <div className="text-center py-8 text-muted-foreground">
                <p>No rows match the current filters.</p>
                <Button
                  variant="link"
                  onClick={() => setFilters([])}
                  className="mt-2"
                >
                  Clear all filters
                </Button>
              </div>
            ) : (
              <NoDataAvailable
                message="No rows to show"
                description={
                  effectiveTable
                    ? "This snapshot has no rows. Try a different load time or table."
                    : "Pick a schema and table to start exploring."
                }
              />
            ))}
        </div>
      </div>
    </div>
  );
}
