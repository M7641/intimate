import { useMemo } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { Bar, BarChart, XAxis, YAxis, CartesianGrid } from "recharts";
import type { ChartConfig } from "@/components/ui/chart";
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from "@/components/ui/chart";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card";
import MTable from "@/components/MTable";
import StatCard from "@/components/StatCard";
import { useTableStorage } from "@/hooks/useSnowflakeData";
import type { TableStorage } from "@/types/snowflake";

// Human-readable size from a GB figure.
function formatGb(gb: number): string {
  if (gb >= 1024) return `${(gb / 1024).toFixed(2)} TB`;
  if (gb >= 1) return `${gb.toFixed(2)} GB`;
  return `${(gb * 1024).toFixed(1)} MB`;
}

interface SchemaRollup {
  schema_name: string;
  table_count: number;
  total_gb: number;
  pct_of_total: number;
}

const tableColumnHelper = createColumnHelper<TableStorage>();
const tableColumns = [
  tableColumnHelper.accessor("schema_name", { header: "Schema", size: 130 }),
  tableColumnHelper.accessor("table_name", { header: "Table", size: 190 }),
  tableColumnHelper.accessor("size_gb", {
    header: "Size",
    size: 100,
    cell: (info) => formatGb(info.getValue()),
  }),
  tableColumnHelper.accessor("row_count", {
    header: "Rows",
    size: 110,
    cell: (info) => info.getValue()?.toLocaleString() ?? "—",
  }),
  tableColumnHelper.accessor("table_type", { header: "Type", size: 120 }),
  tableColumnHelper.accessor("retention_time", {
    header: "Time Travel (d)",
    size: 120,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  tableColumnHelper.accessor("last_altered", {
    header: "Last Altered",
    size: 160,
    cell: (info) => info.getValue()?.slice(0, 10) ?? "—",
  }),
];

const schemaColumnHelper = createColumnHelper<SchemaRollup>();
const schemaColumns = [
  schemaColumnHelper.accessor("schema_name", { header: "Schema", size: 180 }),
  schemaColumnHelper.accessor("table_count", {
    header: "Tables",
    size: 90,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  schemaColumnHelper.accessor("total_gb", {
    header: "Total Size",
    size: 120,
    cell: (info) => formatGb(info.getValue()),
  }),
  schemaColumnHelper.accessor("pct_of_total", {
    header: "% of Total",
    size: 110,
    cell: (info) => `${info.getValue().toFixed(1)}%`,
  }),
];

const chartConfig = {
  total_gb: { label: "Size (GB)", color: "var(--chart-1)" },
} satisfies ChartConfig;

export default function StoragePage() {
  const { data, isLoading } = useTableStorage();
  const rows = data?.data ?? [];

  const { schemaRollup, totalGb, largest } = useMemo(() => {
    const totalMb = rows.reduce((acc, r) => acc + (r.size_mb ?? 0), 0);
    const bySchema = new Map<string, { mb: number; count: number }>();
    for (const r of rows) {
      const cur = bySchema.get(r.schema_name) ?? { mb: 0, count: 0 };
      cur.mb += r.size_mb ?? 0;
      cur.count += 1;
      bySchema.set(r.schema_name, cur);
    }
    const rollup: SchemaRollup[] = Array.from(bySchema.entries())
      .map(([schema_name, { mb, count }]) => ({
        schema_name,
        table_count: count,
        total_gb: mb / 1024,
        pct_of_total: totalMb > 0 ? (mb / totalMb) * 100 : 0,
      }))
      .sort((a, b) => b.total_gb - a.total_gb);
    const largestTable = rows.reduce<TableStorage | null>(
      (max, r) => (max === null || r.size_mb > max.size_mb ? r : max),
      null,
    );
    return {
      schemaRollup: rollup,
      totalGb: totalMb / 1024,
      largest: largestTable,
    };
  }, [rows]);

  const chartData = schemaRollup.slice(0, 10);

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold">Storage</h1>
        <p className="text-muted-foreground">
          Total storage per schema and table (from information_schema.tables) —
          find the largest data sources
        </p>
      </div>

      {/* Summary */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <StatCard label="Total Storage" value={isLoading ? "…" : formatGb(totalGb)} />
        <StatCard label="Schemas" value={isLoading ? "…" : schemaRollup.length.toLocaleString()} />
        <StatCard label="Tables" value={isLoading ? "…" : rows.length.toLocaleString()} />
        <StatCard
          label="Largest Table"
          value={isLoading || !largest ? "—" : formatGb(largest.size_gb)}
          sub={largest ? `${largest.schema_name}.${largest.table_name}` : undefined}
        />
      </div>

      {/* Top schemas chart */}
      <Card>
        <CardHeader>
          <CardTitle>Top Schemas by Size</CardTitle>
          <CardDescription>The heaviest schemas in storage</CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="h-[300px] w-full animate-pulse rounded bg-muted" />
          ) : chartData.length === 0 ? (
            <div className="h-[300px] flex items-center justify-center text-muted-foreground">
              No storage data available
            </div>
          ) : (
            <ChartContainer config={chartConfig} className="h-[300px] w-full">
              <BarChart data={chartData} accessibilityLayer>
                <CartesianGrid vertical={false} />
                <XAxis
                  dataKey="schema_name"
                  tickLine={false}
                  tickMargin={10}
                  axisLine={false}
                  fontSize={12}
                />
                <YAxis
                  tickLine={false}
                  axisLine={false}
                  tickFormatter={(value) =>
                    value >= 1024 ? `${(value / 1024).toFixed(0)}T` : `${value}G`
                  }
                  fontSize={12}
                />
                <ChartTooltip
                  cursor={false}
                  content={<ChartTooltipContent indicator="dashed" />}
                />
                <Bar dataKey="total_gb" fill="var(--color-total_gb)" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ChartContainer>
          )}
        </CardContent>
      </Card>

      {/* Per-schema rollup */}
      <div>
        <h2 className="text-lg font-semibold mb-2">By Schema</h2>
        <div className="bg-card rounded-lg border p-4">
          {isLoading ? (
            <div className="h-48 animate-pulse rounded bg-muted" />
          ) : schemaRollup.length === 0 ? (
            <p className="text-muted-foreground text-center py-8">No storage data available</p>
          ) : (
            <MTable data={schemaRollup} columns={schemaColumns} />
          )}
        </div>
      </div>

      {/* Per-table detail */}
      <div>
        <h2 className="text-lg font-semibold mb-2">By Table</h2>
        <div className="bg-card rounded-lg border p-4">
          {isLoading ? (
            <div className="h-96 animate-pulse rounded bg-muted" />
          ) : rows.length === 0 ? (
            <p className="text-muted-foreground text-center py-8">No storage data available</p>
          ) : (
            <MTable data={rows} columns={tableColumns} />
          )}
        </div>
      </div>
    </div>
  );
}
