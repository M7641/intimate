import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import MTable from "@/components/MTable";
import { useQueryPerformance } from "@/hooks/useRedshiftData";
import type { QueryPerformance } from "@/types/redshift";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const columnHelper = createColumnHelper<QueryPerformance>();

const tableColumns = [
  columnHelper.accessor("schema_name", {
    header: "Schema",
    size: 120,
  }),
  columnHelper.accessor("table_name", {
    header: "Table",
    size: 180,
  }),
  columnHelper.accessor("query_count", {
    header: "Queries",
    size: 80,
    cell: (info) => (info.getValue() || "-").toLocaleString(),
  }),
  columnHelper.accessor("avg_cpu_sec", {
    header: "Avg CPU (s)",
    size: 100,
    cell: (info) => (info.getValue() || 0).toFixed(2),
  }),
  columnHelper.accessor("max_cpu_sec", {
    header: "Max CPU (s)",
    size: 100,
    cell: (info) => (info.getValue() || 0).toFixed(2),
  }),
  columnHelper.accessor("avg_exec_sec", {
    header: "Avg Exec (s)",
    size: 100,
    cell: (info) => (info.getValue() || 0).toFixed(2),
  }),
  columnHelper.accessor("max_exec_sec", {
    header: "Max Exec (s)",
    size: 100,
    cell: (info) => (info.getValue() || 0).toFixed(2),
  }),
  columnHelper.accessor("total_blocks_read", {
    header: "Blocks Read",
    size: 110,
    cell: (info) => (info.getValue() || "-").toLocaleString(),
  }),
  columnHelper.accessor("avg_cpu_usage_pct", {
    header: "Avg CPU %",
    size: 90,
    cell: (info) => `${(info.getValue() || 0).toFixed(1)}%`,
  }),
  columnHelper.accessor("queries_spilled_to_disk", {
    header: "Disk Spills",
    size: 100,
    cell: (info) => (info.getValue() || "-").toLocaleString(),
  }),
];

export default function QueryPerformancePage() {
  const [days, setDays] = useState(30);
  const { data, isLoading } = useQueryPerformance(days);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Query Performance</h1>
          <p className="text-muted-foreground">
            CPU time, execution time, and I/O metrics by table
          </p>
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
              <SelectItem value="7">Last 7 days</SelectItem>
              <SelectItem value="14">Last 14 days</SelectItem>
              <SelectItem value="30">Last 30 days</SelectItem>
              <SelectItem value="60">Last 60 days</SelectItem>
              <SelectItem value="90">Last 90 days</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <div className="bg-card rounded-lg border p-4">
        {isLoading ? (
          <div className="h-96 animate-pulse rounded bg-muted" />
        ) : data?.data?.length === 0 ? (
          <p className="text-muted-foreground text-center py-8">
            No data available
          </p>
        ) : (
          <MTable
            data={data?.data ?? []}
            columns={tableColumns}
            pagination={true}
            paginationSize={20}
          />
        )}
      </div>
    </div>
  );
}
