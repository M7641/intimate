import { useMemo, useState } from "react";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import MTable from "@/components/MTable";
import { useCreditsByTag } from "@/hooks/useSnowflakeData";
import type { CreditsByTag } from "@/types/snowflake";

const fmtCredits = (c: number) => c.toFixed(4);

// ── Per-tag table ─────────────────────────────────────────────────────
const tagColumnHelper = createColumnHelper<CreditsByTag>();
const tagColumns = [
  tagColumnHelper.accessor("tag_source", {
    header: "Source",
    size: 140,
    cell: (info) => info.getValue() ?? "—",
  }),
  tagColumnHelper.accessor("tag_owner", {
    header: "Owner",
    size: 120,
    cell: (info) => info.getValue() ?? "—",
  }),
  tagColumnHelper.accessor("tag_env", {
    header: "Env",
    size: 110,
    cell: (info) => info.getValue() ?? "—",
  }),
  tagColumnHelper.accessor("query_count", {
    header: "Queries",
    size: 100,
    cell: (info) => info.getValue().toLocaleString(),
  }),
  tagColumnHelper.accessor("total_credits", {
    header: "Total Credits",
    size: 130,
    cell: (info) => fmtCredits(info.getValue()),
  }),
  tagColumnHelper.accessor("avg_credits", {
    header: "Avg / Query",
    size: 120,
    cell: (info) => fmtCredits(info.getValue()),
  }),
];

const sourceChartConfig = {
  total_credits: { label: "Credits", color: "var(--chart-2)" },
} satisfies ChartConfig;

export default function CreditSpendPage() {
  const [days, setDays] = useState(7);
  // NOTE: the "Credits per Day" section (useCreditsByDay) is disabled for now —
  // its /credits-by-day Snowflake query is too slow and times out. The backend
  // code is kept; re-enable the hook + card once it runs fast enough.
  const { data: byTag, isLoading: tagLoading } = useCreditsByTag(days);

  const tagRows = byTag?.data ?? [];

  // Credits rolled up by tag source for the breakdown chart.
  const bySource = useMemo(() => {
    const sourceMap = new Map<string, number>();
    for (const t of tagRows) {
      const key = t.tag_source ?? "(untagged)";
      sourceMap.set(key, (sourceMap.get(key) ?? 0) + (t.total_credits ?? 0));
    }
    return Array.from(sourceMap.entries())
      .map(([source, credits]) => ({ source, total_credits: credits }))
      .sort((a, b) => b.total_credits - a.total_credits);
  }, [tagRows]);

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between">
        <div>
          <h1 className="text-2xl font-bold">Credit Spend</h1>
          <p className="text-muted-foreground">
            Estimated compute credits over time and by query tag. Estimated from
            query history; not billed credits.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">Window:</span>
          <Select value={days.toString()} onValueChange={(v) => setDays(Number(v))}>
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

      {/* "Credits per Day" chart is DISABLED — its /credits-by-day query timed
          out (504) historically. It now reads the query-history cache like the
          other views; re-enable the card + useCreditsByDay once confirmed fast. */}

      {/* Credits by source — bound to the by-tag query only. */}
      <Card>
        <CardHeader>
          <CardTitle>Credits by Source</CardTitle>
          <CardDescription>Which application / tag source drives spend</CardDescription>
        </CardHeader>
        <CardContent>
          {tagLoading ? (
            <div className="h-[280px] w-full animate-pulse rounded bg-muted" />
          ) : bySource.length === 0 ? (
            <div className="h-[280px] flex items-center justify-center text-muted-foreground">
              No tagged query history available
            </div>
          ) : (
            <ChartContainer config={sourceChartConfig} className="h-[280px] w-full">
              <BarChart data={bySource} accessibilityLayer>
                <CartesianGrid vertical={false} />
                <XAxis dataKey="source" tickLine={false} tickMargin={10} axisLine={false} fontSize={12} />
                <YAxis tickLine={false} axisLine={false} fontSize={12} />
                <ChartTooltip cursor={false} content={<ChartTooltipContent indicator="dashed" />} />
                <Bar dataKey="total_credits" fill="var(--color-total_credits)" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ChartContainer>
          )}
        </CardContent>
      </Card>

      {/* Per-tag detail — bound to the by-tag query only. */}
      <div>
        <h2 className="text-lg font-semibold mb-2">By Tag</h2>
        <div className="bg-card rounded-lg border p-4">
          {tagLoading ? (
            <div className="h-72 animate-pulse rounded bg-muted" />
          ) : tagRows.length === 0 ? (
            <p className="text-muted-foreground text-center py-8">
              No query history available
            </p>
          ) : (
            <MTable data={tagRows} columns={tagColumns} />
          )}
        </div>
      </div>
    </div>
  );
}
