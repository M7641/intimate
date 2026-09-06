import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";

import MTable from "@/components/MTable";
import { useWarehouseUtilization } from "@/hooks/useSnowflakeData";
import type { WarehouseUtilization } from "@/types/snowflake";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const columnHelper = createColumnHelper<WarehouseUtilization>();

const tableColumns = [
  columnHelper.accessor("warehouse_name", {
    header: "Warehouse",
    size: 150,
  }),
  columnHelper.accessor("warehouse_size", {
    header: "Size",
    size: 80,
  }),
  columnHelper.accessor("query_count", {
    header: "Queries",
    size: 80,
    cell: (info) => (info.getValue() ?? 0).toLocaleString(),
  }),
  columnHelper.accessor("unique_users", {
    header: "Users",
    size: 70,
  }),
  columnHelper.accessor("avg_execution_seconds", {
    header: "Avg Exec (s)",
    size: 100,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
  columnHelper.accessor("p50_execution_seconds", {
    header: "P50 (s)",
    size: 80,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
  columnHelper.accessor("p95_execution_seconds", {
    header: "P95 (s)",
    size: 80,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
  columnHelper.accessor("p99_execution_seconds", {
    header: "P99 (s)",
    size: 80,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
  columnHelper.accessor("avg_queue_seconds", {
    header: "Avg Queue (s)",
    size: 110,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
  columnHelper.accessor("sizing_recommendation", {
    header: "Recommendation",
    size: 250,
    cell: (info) => {
      const rec = info.getValue() ?? "";
      const variant = rec.includes("larger") || rec.includes("multi-cluster")
        ? "destructive"
        : rec.includes("smaller")
        ? "secondary"
        : "default";
      return <Badge variant={variant}>{rec}</Badge>;
    },
  }),
];

export default function WarehouseUtilizationPage() {
  const [days, setDays] = useState(7);
  const { data, isLoading } = useWarehouseUtilization(days);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Warehouse Utilization</h1>
          <p className="text-muted-foreground">
            Warehouse performance metrics and sizing recommendations
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
              <SelectItem value="1">Last 1 day</SelectItem>
              <SelectItem value="3">Last 3 days</SelectItem>
              <SelectItem value="7">Last 7 days</SelectItem>
              <SelectItem value="14">Last 14 days</SelectItem>
              <SelectItem value="30">Last 30 days</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <div className="bg-card rounded-lg border p-4">
        {isLoading ? (
          <div className="h-96 animate-pulse rounded bg-muted" />
        ) : data?.data?.length === 0 ? (
          <p className="text-muted-foreground text-center py-8">
            No warehouse utilization data found
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
