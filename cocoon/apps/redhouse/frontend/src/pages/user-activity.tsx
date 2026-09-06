import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import FilteredTablePage from "@/components/FilteredTablePage";
import { useUserActivity } from "@/hooks/useRedshiftData";
import type { UserActivity } from "@/types/redshift";
import { TIME_RANGE_OPTIONS } from "@/lib/filter-options";

const columnHelper = createColumnHelper<UserActivity>();

const tableColumns = [
  columnHelper.accessor("username", { header: "User", size: 120 }),
  columnHelper.accessor("schema_name", { header: "Schema", size: 120 }),
  columnHelper.accessor("table_name", { header: "Table", size: 180 }),
  columnHelper.accessor("access_count", {
    header: "Accesses",
    size: 100,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("unique_queries", {
    header: "Unique Queries",
    size: 120,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("first_access", {
    header: "First Access",
    size: 160,
    cell: (info) => {
      const val = info.getValue();
      if (!val) return "-";
      return new Date(val).toLocaleString();
    },
  }),
  columnHelper.accessor("last_access", {
    header: "Last Access",
    size: 160,
    cell: (info) => {
      const val = info.getValue();
      if (!val) return "-";
      return new Date(val).toLocaleString();
    },
  }),
];

export default function UserActivityPage() {
  const [days, setDays] = useState(30);
  const { data, isLoading } = useUserActivity(days);

  return (
    <FilteredTablePage
      title="User Activity"
      description="Which users are accessing which tables"
      filters={[
        { label: "Time range:", value: days.toString(), onChange: (v) => setDays(Number(v)), options: TIME_RANGE_OPTIONS },
      ]}
      columns={tableColumns}
      data={data?.data ?? []}
      loading={isLoading}
      emptyMessage="No data available"
    />
  );
}
