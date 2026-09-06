import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import MTable from "@/components/MTable";
import { useDiskQueries } from "@/hooks/useRedshiftData";
import type { DiskQuery } from "@/types/redshift";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const columnHelper = createColumnHelper<DiskQuery>();

const tableColumns = [
  columnHelper.accessor("query", {
    header: "Query ID",
    size: 90,
  }),
  columnHelper.accessor("schema_name", {
    header: "Schema",
    size: 100,
  }),
  columnHelper.accessor("table_name", {
    header: "Table",
    size: 150,
  }),
  columnHelper.accessor("execution_time_sec", {
    header: "Exec (s)",
    size: 80,
    cell: (info) => info.getValue().toFixed(2),
  }),
  columnHelper.accessor("spilled_mb", {
    header: "Spilled (MB)",
    size: 100,
    cell: (info) => info.getValue().toFixed(2),
  }),
  columnHelper.accessor("query_temp_blocks_to_disk", {
    header: "Temp Blocks",
    size: 110,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("rows_scanned", {
    header: "Rows",
    size: 100,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("query_blocks_read", {
    header: "Blocks Read",
    size: 100,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("query_preview", {
    header: "Query Preview",
    size: 300,
    cell: (info) => (
      <span className="text-xs font-mono truncate block max-w-[300px]" title={info.getValue()}>
        {info.getValue()}
      </span>
    ),
  }),
];

export default function DiskQueriesPage() {
  const [days, setDays] = useState(7);
  const { data, isLoading } = useDiskQueries(days);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Disk-Based Queries</h1>
          <p className="text-muted-foreground">
            Queries that spilled to disk - indicates performance issues
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
            No disk-based queries found
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
