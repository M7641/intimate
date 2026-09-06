import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import MTable from "@/components/MTable";
import { useTableScans } from "@/hooks/useRedshiftData";
import type { TableScan } from "@/types/redshift";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const columnHelper = createColumnHelper<TableScan>();

const tableColumns = [
  columnHelper.accessor("schema_name", {
    header: "Schema",
    size: 120,
  }),
  columnHelper.accessor("table_name", {
    header: "Table",
    size: 180,
  }),
  columnHelper.accessor("scan_count", {
    header: "Scans",
    size: 80,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("unique_queries", {
    header: "Queries",
    size: 80,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("unique_users", {
    header: "Users",
    size: 70,
  }),
  columnHelper.accessor("total_rows_scanned", {
    header: "Rows Scanned",
    size: 120,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("total_gb_scanned", {
    header: "GB Scanned",
    size: 100,
    cell: (info) => `${info.getValue().toFixed(2)}`,
  }),
  columnHelper.accessor("filter_efficiency_pct", {
    header: "Filter Eff %",
    size: 100,
    cell: (info) => `${info.getValue().toFixed(1)}%`,
  }),
  columnHelper.accessor("slices_used", {
    header: "Slices",
    size: 70,
  }),
  columnHelper.accessor("days_with_activity", {
    header: "Active Days",
    size: 100,
  }),
];

export default function TableScansPage() {
  const [days, setDays] = useState(30);
  const { data, isLoading } = useTableScans(days);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Table Scan Statistics</h1>
          <p className="text-muted-foreground">
            Detailed scan statistics for all tables
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
