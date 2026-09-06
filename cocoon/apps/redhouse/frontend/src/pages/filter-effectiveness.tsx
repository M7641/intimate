import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import MTable from "@/components/MTable";
import { useFilterEffectiveness } from "@/hooks/useRedshiftData";
import type { FilterEffectiveness } from "@/types/redshift";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const columnHelper = createColumnHelper<FilterEffectiveness>();

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
  columnHelper.accessor("total_rows_returned", {
    header: "Rows Returned",
    size: 120,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("total_rows_scanned", {
    header: "Rows Scanned",
    size: 120,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("avg_filter_efficiency_pct", {
    header: "Avg Filter %",
    size: 100,
    cell: (info) => `${info.getValue().toFixed(1)}%`,
  }),
  columnHelper.accessor("full_table_scans", {
    header: "Full Scans",
    size: 90,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("full_scan_pct", {
    header: "Full Scan %",
    size: 100,
    cell: (info) => {
      const val = info.getValue();
      const variant = val > 50 ? "destructive" : val > 20 ? "secondary" : "outline";
      return <Badge variant={variant}>{val.toFixed(1)}%</Badge>;
    },
  }),
  columnHelper.accessor("filter_assessment", {
    header: "Assessment",
    size: 200,
    cell: (info) => {
      const val = info.getValue();
      let variant: "default" | "destructive" | "secondary" | "outline" = "default";
      if (val.includes("Poor")) variant = "destructive";
      else if (val.includes("Moderate")) variant = "secondary";
      else if (val.includes("Good")) variant = "outline";
      return <Badge variant={variant}>{val}</Badge>;
    },
  }),
];

export default function FilterEffectivenessPage() {
  const [days, setDays] = useState(7);
  const [minScans, setMinScans] = useState(10);
  const { data, isLoading } = useFilterEffectiveness(days, minScans);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Filter Effectiveness</h1>
          <p className="text-muted-foreground">
            How effective filters are for each table - low efficiency may indicate missing WHERE clauses
          </p>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">Min scans:</span>
            <Select
              value={minScans.toString()}
              onValueChange={(v) => setMinScans(Number(v))}
            >
              <SelectTrigger className="w-24">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="5">5+</SelectItem>
                <SelectItem value="10">10+</SelectItem>
                <SelectItem value="25">25+</SelectItem>
                <SelectItem value="50">50+</SelectItem>
                <SelectItem value="100">100+</SelectItem>
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
