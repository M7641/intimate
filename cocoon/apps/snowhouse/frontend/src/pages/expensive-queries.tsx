import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";

import MTable from "@/components/MTable";
import StatCard from "@/components/StatCard";
import PageHeader, { Field } from "@/components/PageHeader";
import { Input } from "@/components/ui/input";
import { useExpensiveQueries } from "@/hooks/useSnowflakeData";
import { formatNumber } from "@/lib/format";
import type { ExpensiveQuery } from "@/types/snowflake";
import QueryDialog from "@/components/QueryDialog";
import { useDebouncedValue } from "@/lib/useDebouncedValue";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const columnHelper = createColumnHelper<ExpensiveQuery>();

const tableColumns = [
  columnHelper.accessor("query_id", {
    header: "Query ID",
    size: 100,
    cell: (info) => {
      const val = info.getValue() ?? "";
      return <span className="font-mono text-xs">{val.slice(0, 8)}...</span>;
    },
  }),
  columnHelper.display({
    id: "full_query",
    header: "Full Query",
    size: 90,
    cell: (info) => <QueryDialog query={info.row.original.query_text} />,
  }),
  columnHelper.accessor("query_type", {
    header: "Type",
    size: 80,
  }),
  columnHelper.accessor("user_name", {
    header: "User",
    size: 120,
  }),
  columnHelper.accessor("warehouse_name", {
    header: "Warehouse",
    size: 140,
  }),
  columnHelper.accessor("warehouse_size", {
    header: "Size",
    size: 80,
  }),
  columnHelper.accessor("estimated_credits", {
    header: "Est. Credits",
    size: 100,
    cell: (info) => (info.getValue() ?? 0).toFixed(6),
  }),
  columnHelper.accessor("execution_seconds", {
    header: "Exec (s)",
    size: 80,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
  columnHelper.accessor("queued_seconds", {
    header: "Queued (s)",
    size: 90,
    cell: (info) => {
      const total = info.getValue() ?? 0;
      const { queued_overload_seconds: overload, queued_provisioning_seconds: provisioning } =
        info.row.original;
      if (total === 0) return <span className="text-muted-foreground">—</span>;
      // Overload = warehouse saturated (contention). Flag it: that's the
      // actionable "add compute / multi-cluster" signal, unlike a cold start.
      return (
        <span
          className={overload > 0 ? "sev-fg-warning font-medium tabular-nums" : "tabular-nums"}
          title={`Overload ${overload.toFixed(2)}s · Provisioning ${provisioning.toFixed(2)}s`}
        >
          {total.toFixed(2)}
        </span>
      );
    },
  }),
  columnHelper.accessor("gb_scanned", {
    header: "GB Scanned",
    size: 100,
    cell: (info) => (info.getValue() ?? 0).toFixed(4),
  }),
  columnHelper.accessor("rows_produced", {
    header: "Rows Created",
    size: 110,
    cell: (info) => formatNumber(info.getValue()),
  }),
  columnHelper.accessor("start_time", {
    header: "Start Time",
    size: 160,
    cell: (info) => {
      const val = info.getValue();
      return val ? new Date(val).toLocaleString() : "—";
    },
  }),
  columnHelper.accessor("query_preview", {
    header: "Query Preview",
    size: 300,
    cell: (info) => {
      const val = info.getValue() ?? "";
      return (
        <span className="text-xs font-mono truncate block max-w-[300px]" title={val}>
          {val}
        </span>
      );
    },
  }),
];

export default function ExpensiveQueriesPage() {
  const [days, setDays] = useState(7);
  const [topN, setTopN] = useState(50);
  const [search, setSearch] = useState("");
  const debouncedSearch = useDebouncedValue(search, 300);
  const { data, isLoading } = useExpensiveQueries(days, topN, debouncedSearch);

  const rows = data?.data ?? [];
  const totalCredits = rows.reduce((acc, q) => acc + (q.estimated_credits ?? 0), 0);

  return (
    <div className="space-y-6">
      <PageHeader
        title="Expensive Queries"
        description={
          <>
            Top queries by estimated compute (execution time × warehouse size),
            from Snowflake query history.
          </>
        }
        actions={
          <>
            <Input
              type="search"
              placeholder="Search query text…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full sm:w-56"
              aria-label="Search query text"
            />
            <Field label="Top N">
              <Select value={topN.toString()} onValueChange={(v) => setTopN(Number(v))}>
                <SelectTrigger className="w-24">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="25">25</SelectItem>
                  <SelectItem value="50">50</SelectItem>
                  <SelectItem value="100">100</SelectItem>
                  <SelectItem value="200">200</SelectItem>
                </SelectContent>
              </Select>
            </Field>
            <Field label="Time range">
              <Select value={days.toString()} onValueChange={(v) => setDays(Number(v))}>
                <SelectTrigger className="w-32">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1">Last 1 day</SelectItem>
                  <SelectItem value="3">Last 3 days</SelectItem>
                  <SelectItem value="7">Last 7 days</SelectItem>
                  <SelectItem value="14">Last 14 days</SelectItem>
                  <SelectItem value="30">Last 30 days</SelectItem>
                </SelectContent>
              </Select>
            </Field>
          </>
        }
      />

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <StatCard
          label={`Total Est. Credits (top ${topN})`}
          value={isLoading ? "…" : totalCredits.toFixed(4)}
          sub="Sum of estimated credits"
        />
        <StatCard
          label="Queries Shown"
          value={isLoading ? "…" : String(rows.length)}
          sub={`Last ${days} day${days === 1 ? "" : "s"}`}
        />
        <StatCard
          label="Priciest Single Query"
          value={isLoading ? "…" : (rows[0]?.estimated_credits ?? 0).toFixed(4)}
          sub={rows[0]?.warehouse_name ?? "—"}
        />
      </div>

      <section className="rounded-xl border bg-card/60 backdrop-blur-sm">
        <div className="flex items-center justify-between border-b px-4 py-3">
          <h2 className="text-sm font-medium">Results</h2>
          {!isLoading && rows.length > 0 && (
            <span className="tabular-nums text-xs text-muted-foreground">
              {rows.length} quer{rows.length === 1 ? "y" : "ies"}
            </span>
          )}
        </div>
        <div className="p-4">
          {isLoading ? (
            <div className="h-96 animate-pulse rounded-lg bg-muted" />
          ) : rows.length === 0 ? (
            <div className="py-16 text-center">
              <p className="text-sm text-muted-foreground">No expensive queries found</p>
              <p className="mt-1 text-xs text-muted-foreground">
                Try a wider time range, or clear the search.
              </p>
            </div>
          ) : (
            <MTable
              data={rows}
              columns={tableColumns}
            />
          )}
        </div>
      </section>
    </div>
  );
}
