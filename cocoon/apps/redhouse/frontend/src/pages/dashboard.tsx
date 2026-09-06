import { createColumnHelper } from "@tanstack/react-table";
import StatCard from "@/components/StatCard";
import QueryFrequencyChart from "@/components/QueryFrequencyChart";
import MTable from "@/components/MTable";
import NoDataAvailable from "@/components/NoDataAvailable";
import {
  useTableScans,
  useSlowQueries,
  useUnusedTables,
  useQueryFrequency,
} from "@/hooks/useRedshiftData";
import type { TableScan } from "@/types/redshift";
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
  columnHelper.accessor("total_gb_scanned", {
    header: "GB Scanned",
    size: 100,
    cell: (info) => `${info.getValue().toFixed(2)} GB`,
  }),
  columnHelper.accessor("filter_efficiency_pct", {
    header: "Filter Eff.",
    size: 100,
    cell: (info) => `${info.getValue().toFixed(1)}%`,
  }),
  columnHelper.accessor("days_with_activity", {
    header: "Active Days",
    size: 100,
  }),
];

export default function DashboardPage() {
  const { data: tableScans, isLoading: scansLoading } = useTableScans(30);
  const { data: slowQueries, isLoading: slowLoading } = useSlowQueries(10, 7);
  const { data: unusedTables, isLoading: unusedLoading } = useUnusedTables(30);
  const { data: queryFrequency, isLoading: freqLoading } = useQueryFrequency(7);

  const totalTables = tableScans?.data?.length ?? 0;
  const totalSlowQueries = slowQueries?.data?.length ?? 0;
  const totalUnusedTables = unusedTables?.data?.length ?? 0;
  const totalScans =
    tableScans?.data?.reduce((acc, t) => acc + t.scan_count, 0) ?? 0;

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-bold">Redshift Overview</h1>
        <p className="text-muted-foreground">
          Scan volume, slow queries, and unused tables across recent activity
        </p>
      </header>

      {/* Stat Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          label="Active Tables"
          value={scansLoading ? "…" : String(totalTables)}
          sub="Tables with activity in last 30 days"
        />
        <StatCard
          label="Total Scans"
          value={scansLoading ? "…" : totalScans.toLocaleString()}
          sub="Table scans in last 30 days"
        />
        <StatCard
          label="Slow Queries"
          value={slowLoading ? "…" : String(totalSlowQueries)}
          sub="Queries taking >10s (last 7 days)"
        />
        <StatCard
          label="Potentially Unused Tables"
          value={unusedLoading ? "…" : String(totalUnusedTables)}
          sub="No activity in 30+ days"
        />
      </div>

      {/* Query Frequency Chart */}
      <QueryFrequencyChart
        data={queryFrequency?.data ?? []}
        loading={freqLoading}
        dataKey="total_scans"
        label="Total Scans"
      />

      {/* Top Tables Table */}
      <div className="bg-card rounded-lg border p-4">
        <h2 className="text-lg font-semibold mb-4">
          Top Tables by Scan Count
        </h2>
        {scansLoading ? (
          <div className="h-48 animate-pulse rounded bg-muted" />
        ) : tableScans?.data?.length === 0 ? (
          <NoDataAvailable message="No table activity" description="No tables were scanned in the last 30 days." />
        ) : (
          <MTable
            data={tableScans?.data?.slice(0, 20) ?? []}
            columns={tableColumns}
            pagination={false}
          />
        )}
      </div>
    </div>
  );
}
