import { createColumnHelper } from "@tanstack/react-table";
import MTable from "@/components/MTable";
import { useCompression } from "@/hooks/useRedshiftData";
import type { TableCompression } from "@/types/redshift";
import { Badge } from "@/components/ui/badge";

const columnHelper = createColumnHelper<TableCompression>();

const tableColumns = [
  columnHelper.accessor("schema_name", {
    header: "Schema",
    size: 120,
  }),
  columnHelper.accessor("table_name", {
    header: "Table",
    size: 180,
  }),
  columnHelper.accessor("total_columns", {
    header: "Columns",
    size: 80,
  }),
  columnHelper.accessor("uncompressed_cols", {
    header: "Uncompressed",
    size: 110,
  }),
  columnHelper.accessor("uncompressed_pct", {
    header: "Uncomp %",
    size: 90,
    cell: (info) => {
      const val = info.getValue();
      const color = val > 50 ? "destructive" : val > 20 ? "secondary" : "outline";
      return <Badge variant={color}>{val.toFixed(1)}%</Badge>;
    },
  }),
  columnHelper.accessor("az64_cols", {
    header: "AZ64",
    size: 60,
  }),
  columnHelper.accessor("zstd_cols", {
    header: "ZSTD",
    size: 60,
  }),
  columnHelper.accessor("lzo_cols", {
    header: "LZO",
    size: 60,
  }),
  columnHelper.accessor("bytedict_cols", {
    header: "ByteDict",
    size: 80,
  }),
  columnHelper.accessor("sort_key", {
    header: "Sort Key",
    size: 120,
    cell: (info) => info.getValue() ?? "-",
  }),
  columnHelper.accessor("dist_key", {
    header: "Dist Key",
    size: 120,
    cell: (info) => info.getValue() ?? "-",
  }),
];

export default function CompressionPage() {
  const { data, isLoading } = useCompression();

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold">Compression Analysis</h1>
        <p className="text-muted-foreground">
          Compression encodings used across tables - identifies optimization opportunities
        </p>
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
