import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";

import MTable from "@/components/MTable";
import StatCard from "@/components/StatCard";
import PageHeader, { Field } from "@/components/PageHeader";
import { Input } from "@/components/ui/input";
import { useCostByTable } from "@/hooks/useSnowflakeData";
import { formatNumber } from "@/lib/format";
import { useDebouncedValue } from "@/lib/useDebouncedValue";
import type { CostByTable } from "@/types/snowflake";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const columnHelper = createColumnHelper<CostByTable>();

const tableColumns = [
  columnHelper.accessor("target_table", {
    header: "Table",
    size: 280,
    cell: (info) => (
      <span className="font-mono text-xs" title={info.getValue()}>
        {info.getValue()}
      </span>
    ),
  }),
  columnHelper.accessor("total_credits", {
    header: "Est. Credits",
    size: 110,
    cell: (info) => (info.getValue() ?? 0).toFixed(4),
  }),
  columnHelper.accessor("write_count", {
    header: "Writes",
    size: 90,
    cell: (info) => formatNumber(info.getValue()),
  }),
  columnHelper.accessor("total_rows_created", {
    header: "Rows Created",
    size: 120,
    cell: (info) => formatNumber(info.getValue()),
  }),
  columnHelper.accessor("avg_rows_created", {
    header: "Avg Rows",
    size: 110,
    cell: (info) => formatNumber(info.getValue()),
  }),
  columnHelper.accessor("total_gb_scanned", {
    header: "GB Scanned",
    size: 110,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
  columnHelper.accessor("total_elapsed_seconds", {
    header: "Total Exec (s)",
    size: 120,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
];

export default function CostByTablePage() {
  const [days, setDays] = useState(7);
  const [search, setSearch] = useState("");
  const debouncedSearch = useDebouncedValue(search, 300);
  const { data, isLoading } = useCostByTable(days, 100, debouncedSearch);

  const rows = data?.data ?? [];
  const totalCredits = rows.reduce((acc, t) => acc + (t.total_credits ?? 0), 0);

  return (
    <div className="space-y-6">
      <PageHeader
        title="Cost by Table"
        description="Estimated compute credits attributed to each table's write statements — parsed from the SQL (CREATE, MERGE, INSERT, COPY, UPDATE, DELETE, TRUNCATE). Read-only queries are excluded."
        actions={
          <>
            <Input
              type="search"
              placeholder="Search table name…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full sm:w-56"
              aria-label="Search table name"
            />
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

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <StatCard
          label="Total Est. Credits"
          value={isLoading ? "…" : totalCredits.toFixed(4)}
          sub="Sum across shown tables"
        />
        <StatCard
          label="Tables"
          value={isLoading ? "…" : String(rows.length)}
          sub={`With writes in the last ${days} day${days === 1 ? "" : "s"}`}
        />
        <StatCard
          label="Priciest Table"
          value={isLoading ? "…" : (rows[0]?.total_credits ?? 0).toFixed(4)}
          sub={rows[0]?.target_table ?? "—"}
        />
      </div>

      <section className="rounded-xl border bg-card/60 backdrop-blur-sm">
        <div className="flex items-center justify-between border-b px-4 py-3">
          <h2 className="text-sm font-medium">Results</h2>
          {!isLoading && rows.length > 0 && (
            <span className="tabular-nums text-xs text-muted-foreground">
              {rows.length} table{rows.length === 1 ? "" : "s"}
            </span>
          )}
        </div>
        <div className="p-4">
          {isLoading ? (
            <div className="h-96 animate-pulse rounded-lg bg-muted" />
          ) : rows.length === 0 ? (
            <div className="py-16 text-center">
              <p className="text-sm text-muted-foreground">No table writes found</p>
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
