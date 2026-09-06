import { useEffect, useMemo, useState } from "react";
import { Link, useSearch } from "@tanstack/react-router";
import { TrendingUp, TrendingDown, Minus } from "lucide-react";

import DataLoading from "@/components/DataLoad";
import FetchingIndicator from "@/components/FetchingIndicator";
import StatCard from "@/components/StatCard";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useDataExplorerSchemas, useSchemaHealth } from "@/pages/_shared/api";
import type { TableHealth } from "@/pages/_shared/types";
import { formatNumber, formatRelativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";

// A table not reloaded in this many days is flagged stale. Fixed heuristic:
// we don't know each table's own cadence, so we label it plainly.
const STALE_DAYS = 7;

/** Whole days since a load timestamp, or null when never loaded. */
function ageInDays(lastLoaded: string | null): number | null {
  if (!lastLoaded) return null;
  const ms = Date.now() - new Date(lastLoaded).getTime();
  if (Number.isNaN(ms)) return null;
  return ms / 86_400_000;
}

type SortKey = "stale" | "name" | "rows" | "change";

const SORT_LABELS: Record<SortKey, string> = {
  stale: "Most stale",
  name: "Name",
  rows: "Most rows",
  change: "Biggest change",
};

/** Freshness dot + relative time. Colour encodes how worried to be. */
function FreshnessCell({ lastLoaded }: { lastLoaded: string | null }) {
  const tone = (() => {
    const age = ageInDays(lastLoaded);
    if (age === null) return "bg-destructive"; // never loaded / empty
    if (age < 1) return "bg-emerald-500";
    if (age <= STALE_DAYS) return "bg-muted-foreground";
    return "bg-amber-500";
  })();
  return (
    <span className="flex items-center gap-2">
      <span className={`inline-block size-2 rounded-full ${tone}`} />
      <span>{formatRelativeTime(lastLoaded)}</span>
    </span>
  );
}

/** Volume trend: arrow + signed delta percentage. */
function TrendCell({ delta }: { delta: number | null }) {
  if (delta === null) return <span className="text-muted-foreground">—</span>;
  if (Math.abs(delta) < 0.05) {
    return (
      <span className="flex items-center gap-1 text-muted-foreground">
        <Minus className="size-3.5" /> 0%
      </span>
    );
  }
  return (
    <span
      className={cn(
        "flex items-center gap-1",
        delta > 0 && "text-emerald-600 dark:text-emerald-400",
        delta < 0 && "text-destructive",
      )}
    >
      {delta > 0 ? (
        <TrendingUp className="size-3.5" />
      ) : (
        <TrendingDown className="size-3.5" />
      )}
      {delta > 0 ? "+" : ""}
      {delta.toFixed(1)}%
    </span>
  );
}

export default function SchemaHealthPage() {
  const search = useSearch({ strict: false }) as { schema?: string };
  const [selectedSchema, setSelectedSchema] = useState<string>(
    search.schema ?? "",
  );
  const [sortKey, setSortKey] = useState<SortKey>("stale");

  const schemas = useDataExplorerSchemas();
  useEffect(() => {
    const list = schemas.data;
    if (list && list.length > 0 && !selectedSchema) {
      setSelectedSchema(list.includes("stage") ? "stage" : list[0]);
    }
  }, [schemas.data, selectedSchema]);

  const health = useSchemaHealth(selectedSchema);
  // First load = we have nothing to show yet (no data, no error). True while the
  // query is still disabled (schema not yet picked) and during the first fetch,
  // false once data/error arrives. We avoid `isPending` here: its value for a
  // *disabled* query is subtle, and "nothing yet" is what the skeleton means.
  const firstLoad = !health.data && !health.error;
  // A refetch with data already on screen: keep the stale rows but signal the
  // refresh (spinner + dim) so a click isn't met with dead silence.
  const updating = health.isFetching && !!health.data;

  const summary = useMemo(() => {
    const tables = health.data?.tables;
    if (!tables) return null;
    let stale = 0;
    let empty = 0;
    let totalRows = 0;
    for (const t of tables) {
      totalRows += t.current_rows;
      if (t.current_rows === 0) empty += 1;
      const age = ageInDays(t.last_loaded);
      if (age === null || age > STALE_DAYS) stale += 1;
    }
    return { count: tables.length, stale, empty, totalRows };
  }, [health.data]);

  const sortedTables = useMemo<TableHealth[]>(() => {
    const tables = [...(health.data?.tables ?? [])];
    switch (sortKey) {
      case "name":
        return tables.sort((a, b) => a.table_name.localeCompare(b.table_name));
      case "rows":
        return tables.sort((a, b) => b.current_rows - a.current_rows);
      case "change":
        return tables.sort(
          (a, b) =>
            Math.abs(b.row_delta_pct ?? 0) - Math.abs(a.row_delta_pct ?? 0),
        );
      case "stale":
      default:
        // Oldest (and never-loaded) first — surface problems at the top.
        return tables.sort((a, b) => {
          const aa = ageInDays(a.last_loaded);
          const ba = ageInDays(b.last_loaded);
          if (aa === null) return ba === null ? 0 : -1;
          if (ba === null) return 1;
          return ba - aa;
        });
    }
  }, [health.data, sortKey]);

  return (
    <div className="flex flex-col flex-1 overflow-hidden">
      {/* Top bar */}
      <div className="flex items-center gap-4 px-4 py-2 border-b shrink-0">
        <Select value={selectedSchema} onValueChange={setSelectedSchema}>
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
          value={sortKey}
          onValueChange={(v) => setSortKey(v as SortKey)}
        >
          <SelectTrigger className="w-44">
            <SelectValue placeholder="Sort by" />
          </SelectTrigger>
          <SelectContent>
            {(["stale", "name", "rows", "change"] as SortKey[]).map((k) => (
              <SelectItem key={k} value={k}>
                {SORT_LABELS[k]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <FetchingIndicator when={updating} className="ml-auto" />
      </div>

      {/* Content. Dim the stale rows while a refetch is in flight so a schema
          switch reads as "refreshing", not "frozen". */}
      <div
        className={cn(
          "flex-1 overflow-auto p-4 space-y-6",
          updating && "opacity-60 transition-opacity",
        )}
      >
        <DataLoading
          isPending={firstLoad}
          error={health.error}
          onRetry={() => health.refetch()}
          variant="page"
        />

        {summary && (
          <>
            {/* Summary */}
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
              <StatCard label="Tables" value={formatNumber(summary.count)} />
              <StatCard
                label="Stale"
                value={formatNumber(summary.stale)}
                sub={`Not loaded in ${STALE_DAYS}+ days`}
              />
              <StatCard
                label="Empty"
                value={formatNumber(summary.empty)}
                sub="No rows in latest load"
              />
              <StatCard
                label="Total rows"
                value={formatNumber(summary.totalRows)}
                sub="Across latest snapshots"
              />
            </div>

            {/* Health table */}
            {sortedTables.length > 0 ? (
              <div className="rounded-lg border overflow-hidden">
                <table className="min-w-full divide-y text-sm">
                  <thead className="bg-muted/40">
                    <tr className="text-left text-xs font-semibold text-muted-foreground">
                      <th className="px-3 py-2">Table</th>
                      <th className="px-3 py-2">Last loaded</th>
                      <th className="px-3 py-2 text-right">Rows</th>
                      <th className="px-3 py-2 text-right">Trend</th>
                      <th className="px-3 py-2 text-right">Snapshots</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y">
                    {sortedTables.map((t) => (
                      <tr key={t.table_name} className="hover:bg-accent/40">
                        <td className="px-3 py-2 font-medium">
                          <Link
                            to="/table-info"
                            search={{
                              schema: selectedSchema,
                              table: t.table_name,
                            }}
                            className="font-mono hover:text-primary hover:underline"
                          >
                            {t.table_name}
                          </Link>
                        </td>
                        <td className="px-3 py-2">
                          <FreshnessCell lastLoaded={t.last_loaded} />
                        </td>
                        <td className="px-3 py-2 text-right tabular-nums">
                          {formatNumber(t.current_rows)}
                        </td>
                        <td className="px-3 py-2 text-right">
                          <span className="inline-flex justify-end tabular-nums">
                            <TrendCell delta={t.row_delta_pct} />
                          </span>
                        </td>
                        <td className="px-3 py-2 text-right tabular-nums text-muted-foreground">
                          {formatNumber(t.snapshot_count)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">
                No event-sourced tables found in this schema.
              </p>
            )}
          </>
        )}
      </div>
    </div>
  );
}
