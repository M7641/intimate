import { useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import FilteredTablePage from "@/components/FilteredTablePage";
import { useUnusedTables } from "@/hooks/useRedshiftData";
import type { UnusedTable } from "@/types/redshift";
import { Badge } from "@/components/ui/badge";
import { INACTIVE_DAYS_OPTIONS } from "@/lib/filter-options";

const columnHelper = createColumnHelper<UnusedTable>();

const tableColumns = [
  columnHelper.accessor("schema_name", { header: "Schema", size: 120 }),
  columnHelper.accessor("table_name", { header: "Table", size: 200 }),
  columnHelper.accessor("column_count", { header: "Columns", size: 80 }),
  columnHelper.accessor("has_sort_key", {
    header: "Sort Key",
    size: 90,
    cell: (info) => (info.getValue() > 0 ? "Yes" : "No"),
  }),
  columnHelper.accessor("has_dist_key", {
    header: "Dist Key",
    size: 90,
    cell: (info) => (info.getValue() > 0 ? "Yes" : "No"),
  }),
  columnHelper.accessor("last_accessed", { header: "Last Accessed", size: 180 }),
  columnHelper.accessor("status", {
    header: "Status",
    size: 140,
    cell: (info) => {
      const status = info.getValue();
      const variant = status === "Potentially Unused" ? "destructive" : "secondary";
      return <Badge variant={variant}>{status}</Badge>;
    },
  }),
];

export default function UnusedTablesPage() {
  const [daysThreshold, setDaysThreshold] = useState(30);
  const { data, isLoading } = useUnusedTables(daysThreshold);

  return (
    <FilteredTablePage
      title="Unused Tables"
      description="Tables that haven't been accessed recently - candidates for archival"
      filters={[
        { label: "Inactive for:", value: daysThreshold.toString(), onChange: (v) => setDaysThreshold(Number(v)), options: INACTIVE_DAYS_OPTIONS },
      ]}
      columns={tableColumns}
      data={data?.data ?? []}
      loading={isLoading}
      emptyMessage="No unused tables found"
    />
  );
}
