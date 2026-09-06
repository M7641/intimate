import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";

import MTable from "@/components/MTable";
import QueryDialog from "@/components/QueryDialog";
import { useRepeatedQueries } from "@/hooks/useSnowflakeData";
import type { MatchMode, RepeatedQuery } from "@/types/snowflake";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const columnHelper = createColumnHelper<RepeatedQuery>();

const tableColumns = [
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
  columnHelper.accessor("execution_count", {
    header: "Executions",
    size: 100,
    cell: (info) => (info.getValue() ?? 0).toLocaleString(),
  }),
  columnHelper.accessor("unique_users", {
    header: "Users",
    size: 70,
  }),
  columnHelper.accessor("warehouses_used", {
    header: "Warehouses",
    size: 100,
  }),
  columnHelper.accessor("avg_execution_seconds", {
    header: "Avg Exec (s)",
    size: 110,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
  columnHelper.accessor("total_execution_seconds", {
    header: "Total Exec (s)",
    size: 120,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
  columnHelper.accessor("total_gb_scanned", {
    header: "Total GB Scanned",
    size: 140,
    cell: (info) => (info.getValue() ?? 0).toFixed(4),
  }),
  columnHelper.accessor("estimated_total_credits", {
    header: "Est. Credits",
    size: 110,
    cell: (info) => (info.getValue() ?? 0).toFixed(6),
  }),
];

export default function RepeatedQueriesPage() {
  const [days, setDays] = useState(7);
  const [minCount, setMinCount] = useState(5);
  const [mode, setMode] = useState<MatchMode>("fuzzy");
  const { data, isLoading } = useRepeatedQueries(days, minCount, mode);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Repeated Queries</h1>
          <p className="text-muted-foreground">
            {mode === "fuzzy"
              ? "Queries of the same shape (ignoring literal values) run repeatedly — caching and materialized view candidates"
              : "Byte-for-byte identical queries run repeatedly — caching and materialized view candidates"}
          </p>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">Match:</span>
            <Select value={mode} onValueChange={(v) => setMode(v as MatchMode)}>
              <SelectTrigger className="w-28">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="fuzzy">Fuzzy</SelectItem>
                <SelectItem value="exact">Exact</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">Min reps:</span>
            <Select
              value={minCount.toString()}
              onValueChange={(v) => setMinCount(Number(v))}
            >
              <SelectTrigger className="w-24">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="2">2</SelectItem>
                <SelectItem value="5">5</SelectItem>
                <SelectItem value="10">10</SelectItem>
                <SelectItem value="25">25</SelectItem>
                <SelectItem value="50">50</SelectItem>
                <SelectItem value="100">100</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">Time range:</span>
            <Select
              value={days.toString()}
              onValueChange={(v) => setDays(Number(v))}
            >
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
          </div>
        </div>
      </div>

      <div className="bg-card rounded-lg border p-4">
        {isLoading ? (
          <div className="h-96 animate-pulse rounded bg-muted" />
        ) : data?.data?.length === 0 ? (
          <p className="text-muted-foreground text-center py-8">
            No repeated queries found
          </p>
        ) : (
          <MTable
            data={data?.data ?? []}
            columns={tableColumns}
          />
        )}
      </div>
    </div>
  );
}
