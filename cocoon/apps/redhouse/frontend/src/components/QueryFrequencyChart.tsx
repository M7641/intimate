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

interface QueryFrequencyChartProps {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  data: Array<{ hour_of_day: number } & Record<string, any>>;
  loading?: boolean;
  dataKey?: string;
  label?: string;
}

function formatHour(hour: number): string {
  if (hour === 0) return "12 AM";
  if (hour === 12) return "12 PM";
  return hour < 12 ? `${hour} AM` : `${hour - 12} PM`;
}

export default function QueryFrequencyChart({
  data,
  loading = false,
  dataKey = "total_queries",
  label = "Total Queries",
}: QueryFrequencyChartProps) {
  const chartConfig = {
    [dataKey]: {
      label,
      color: "var(--chart-1)",
    },
    unique_queries: {
      label: "Unique Queries",
      color: "var(--chart-2)",
    },
  } satisfies ChartConfig;

  const chartData = data.map((item) => ({
    ...item,
    hour: formatHour(item.hour_of_day),
  }));

  return (
    <Card>
      <CardHeader>
        <CardTitle>Query Frequency by Hour</CardTitle>
        <CardDescription>
          Number of queries by hour of day
        </CardDescription>
      </CardHeader>
      <CardContent>
        {loading ? (
          <div className="h-[300px] w-full animate-pulse rounded bg-muted" />
        ) : data.length === 0 ? (
          <div className="h-[300px] flex items-center justify-center text-muted-foreground">
            No data available
          </div>
        ) : (
          <ChartContainer config={chartConfig} className="h-[300px] w-full">
            <BarChart data={chartData} accessibilityLayer>
              <CartesianGrid vertical={false} />
              <XAxis
                dataKey="hour"
                tickLine={false}
                tickMargin={10}
                axisLine={false}
                fontSize={12}
              />
              <YAxis
                tickLine={false}
                axisLine={false}
                tickFormatter={(value) =>
                  value >= 1000 ? `${(value / 1000).toFixed(0)}k` : value
                }
                fontSize={12}
              />
              <ChartTooltip
                cursor={false}
                content={<ChartTooltipContent indicator="dashed" />}
              />
              <Bar
                dataKey={dataKey}
                fill={`var(--color-${dataKey})`}
                radius={[4, 4, 0, 0]}
              />
            </BarChart>
          </ChartContainer>
        )}
      </CardContent>
    </Card>
  );
}
