import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import FilteredTablePage from "@/components/FilteredTablePage";
import { useSlowQueries } from "@/hooks/useRedshiftData";
import type { SlowQuery } from "@/types/redshift";
import { TIME_RANGE_OPTIONS, MIN_SECONDS_OPTIONS } from "@/lib/filter-options";

const columnHelper = createColumnHelper<SlowQuery>();

const tableColumns = [
  columnHelper.accessor("query", { header: "Query ID", size: 90 }),
  columnHelper.accessor("schema_name", { header: "Schema", size: 100 }),
  columnHelper.accessor("table_name", { header: "Table", size: 150 }),
  columnHelper.accessor("username", { header: "User", size: 100 }),
  columnHelper.accessor("execution_time_sec", {
    header: "Exec (s)",
    size: 80,
    cell: (info) => info.getValue().toFixed(2),
  }),
  columnHelper.accessor("cpu_time_sec", {
    header: "CPU (s)",
    size: 80,
    cell: (info) => info.getValue().toFixed(2),
  }),
  columnHelper.accessor("rows_scanned", {
    header: "Rows",
    size: 100,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("filter_efficiency_pct", {
    header: "Filter %",
    size: 80,
    cell: (info) => `${info.getValue().toFixed(1)}%`,
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

export default function SlowQueriesPage() {
  const [days, setDays] = useState(7);
  const [minSeconds, setMinSeconds] = useState(10);
  const { data, isLoading } = useSlowQueries(minSeconds, days);

  return (
    <FilteredTablePage
      title="Slow Queries"
      description="Queries that exceeded the minimum execution time threshold"
      filters={[
        { label: "Min seconds:", value: minSeconds.toString(), onChange: (v) => setMinSeconds(Number(v)), options: MIN_SECONDS_OPTIONS, width: "w-24" },
        { label: "Time range:", value: days.toString(), onChange: (v) => setDays(Number(v)), options: TIME_RANGE_OPTIONS },
      ]}
      columns={tableColumns}
      data={data?.data ?? []}
      loading={isLoading}
      emptyMessage="No slow queries found"
    />
  );
}
