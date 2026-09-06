import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";

import MTable from "@/components/MTable";
import { useFailedQueries } from "@/hooks/useSnowflakeData";
import type { FailedQuery } from "@/types/snowflake";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const columnHelper = createColumnHelper<FailedQuery>();

const tableColumns = [
  columnHelper.accessor("error_code", {
    header: "Error Code",
    size: 100,
    cell: (info) => (info.getValue() ?? "—"),
  }),
  columnHelper.accessor("error_message", {
    header: "Error Message",
    size: 280,
    cell: (info) => {
      const val = info.getValue() ?? "";
      return (
        <span className="text-xs truncate block max-w-[280px]" title={val}>
          {val || "—"}
        </span>
      );
    },
  }),
  columnHelper.accessor("query_type", {
    header: "Query Type",
    size: 100,
  }),
  columnHelper.accessor("user_name", {
    header: "User",
    size: 120,
  }),
  columnHelper.accessor("warehouse_name", {
    header: "Warehouse",
    size: 130,
    cell: (info) => (info.getValue() ?? "—"),
  }),
  columnHelper.accessor("failure_count", {
    header: "Failures",
    size: 90,
    cell: (info) => (info.getValue() ?? 0).toLocaleString(),
  }),
  columnHelper.accessor("first_failure", {
    header: "First Failure",
    size: 160,
    cell: (info) => {
      const val = info.getValue();
      return val ? new Date(val).toLocaleString() : "—";
    },
  }),
  columnHelper.accessor("last_failure", {
    header: "Last Failure",
    size: 160,
    cell: (info) => {
      const val = info.getValue();
      return val ? new Date(val).toLocaleString() : "—";
    },
  }),
];

export default function FailedQueriesPage() {
  const [days, setDays] = useState(7);
  const { data, isLoading } = useFailedQueries(days);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Failed Queries</h1>
          <p className="text-muted-foreground">
            Error patterns grouped by error code, user, and warehouse
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
            No failed queries found
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
