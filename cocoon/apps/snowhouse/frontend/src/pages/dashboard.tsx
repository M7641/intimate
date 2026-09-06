import { createColumnHelper } from "@tanstack/react-table";

import StatCard from "@/components/StatCard";
import QueryFrequencyChart from "@/components/QueryFrequencyChart";
import MTable from "@/components/MTable";
import NoDataAvailable from "@/components/NoDataAvailable";
import {
  useExpensiveQueries,
  useCostByQueryType,
  useWarehouseUtilization,
  useTableStorage,
  useQueryFrequency,
  useFailedQueries,
} from "@/hooks/useSnowflakeData";
import type { WarehouseUtilization } from "@/types/snowflake";
const columnHelper = createColumnHelper<WarehouseUtilization>();

const tableColumns = [
  columnHelper.accessor("warehouse_name", {
    header: "Warehouse",
    size: 180,
  }),
  columnHelper.accessor("warehouse_size", {
    header: "Size",
    size: 100,
  }),
  columnHelper.accessor("query_count", {
    header: "Queries",
    size: 100,
  }),
  columnHelper.accessor("avg_execution_seconds", {
    header: "Avg Exec (s)",
    size: 120,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
  columnHelper.accessor("p95_execution_seconds", {
    header: "P95 Exec (s)",
    size: 120,
    cell: (info) => (info.getValue() ?? 0).toFixed(2),
  }),
  columnHelper.accessor("sizing_recommendation", {
    header: "Recommendation",
    size: 250,
  }),
];

export default function DashboardPage() {
  const { data: expensiveQueries, isLoading: expensiveLoading } =
    useExpensiveQueries(7);
  const { data: costByType, isLoading: costLoading } = useCostByQueryType(7);
  const { data: warehouseUtil, isLoading: utilLoading } =
    useWarehouseUtilization(7);
  const { data: tableStorage, isLoading: storageLoading } = useTableStorage();
  const { data: queryFrequency, isLoading: freqLoading } = useQueryFrequency(7);
  const { data: failedQueries, isLoading: failedLoading } = useFailedQueries(7);

  // Total across ALL queries in the window (cost-by-query-type groups every
  // query by type with no row cap), not just the top-50 expensive ones.
  const totalQueryCost =
    costByType?.data?.reduce(
      (acc, c) => acc + (c.estimated_compute_credits ?? 0),
      0,
    ) ?? 0;
  const activeWarehouses = warehouseUtil?.data?.length ?? 0;
  const longQueries =
    expensiveQueries?.data?.filter((q) => (q.execution_seconds ?? 0) > 60)
      .length ?? 0;
  const totalStorageGB =
    tableStorage?.data?.reduce((acc, t) => acc + (t.size_gb ?? 0), 0) ?? 0;
  const totalFailures =
    failedQueries?.data?.reduce((acc, f) => acc + (f.failure_count ?? 0), 0) ??
    0;
  const totalQueries =
    queryFrequency?.data?.reduce((acc, h) => acc + (h.total_queries ?? 0), 0) ??
    0;

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-bold">Snowflake Overview</h1>
        <p className="text-muted-foreground">
          Cost, utilization, and query health across the last 7 days
        </p>
      </header>

      {/* Stat Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6 gap-4">
        <StatCard
          label="Total Queries (7d)"
          value={freqLoading ? "…" : totalQueries.toLocaleString()}
          sub="Successful queries in the last 7 days"
        />
        <StatCard
          label="Total Query Cost (7d)"
          value={costLoading ? "…" : totalQueryCost.toFixed(4)}
          sub="Estimated credits across all queries"
        />
        <StatCard
          label="Active Warehouses"
          value={utilLoading ? "…" : String(activeWarehouses)}
          sub="Warehouses with activity"
        />
        <StatCard
          label="Long Queries (>60s)"
          value={expensiveLoading ? "…" : String(longQueries)}
          sub="Queries exceeding 60s (last 7 days)"
        />
        <StatCard
          label="Total Storage"
          value={
            storageLoading
              ? "…"
              : totalStorageGB >= 1024
                ? `${(totalStorageGB / 1024).toFixed(2)} TB`
                : `${totalStorageGB.toFixed(2)} GB`
          }
          sub="Total table storage"
        />
        <StatCard
          label="Failed Queries (7d)"
          value={failedLoading ? "…" : totalFailures.toLocaleString()}
          sub="Total query failures"
        />
      </div>

      {/* Snowflake cost model */}
      <section className="bg-card rounded-lg border p-6 space-y-4">
        <div>
          <h2 className="text-lg font-semibold">Snowflake cost model</h2>
          <p className="text-sm text-muted-foreground">
            How compute credits become dollars — the basis for every cost
            estimate in this app.
          </p>
        </div>
        <ol className="list-decimal space-y-2 pl-5 text-sm">
          <li>
            <span className="font-medium">Minimum 60s billing.</span> Each time
            a warehouse resumes, it bills at least 60 seconds of compute — a
            near-instant query is never free.
          </li>
          <li>
            <span className="font-medium">Both warehouses are Large</span> ={" "}
            <span className="tabular-nums">8 credits/hour</span>.
          </li>
          <li>
            <span className="font-medium">Enterprise rate</span> ={" "}
            <span className="tabular-nums">$3 per credit</span>.
          </li>
        </ol>
        <div className="rounded-md bg-muted p-4 text-sm space-y-1">
          <p className="font-medium">Worked example</p>
          <p>
            A 10-minute query on a Large warehouse: 8 credits/hr × (10 ÷ 60) hr
            = <span className="font-semibold tabular-nums">1.33 credits</span> →
            1.33 × $3 ={" "}
            <span className="font-semibold tabular-nums">$4.00</span>.
          </p>
          <p className="text-muted-foreground">
            Even a near-instant query still bills the 60s minimum: 8 × (60 ÷
            3600) = <span className="tabular-nums">0.13 credits</span> →{" "}
            <span className="font-semibold tabular-nums">$0.40</span>.
          </p>
        </div>
      </section>

      {/* Snowflake storage cost model */}
      <section className="bg-card rounded-lg border p-6 space-y-4">
        <div>
          <h2 className="text-lg font-semibold">Storage cost model</h2>
          <p className="text-sm text-muted-foreground">
            How stored bytes become dollars — billed separately from, and
            differently to, compute credits.
          </p>
        </div>
        <ol className="list-decimal space-y-2 pl-5 text-sm">
          <li>
            <span className="font-medium">Flat rate per TB / month</span>,
            charged on the average daily bytes stored (Snowflake compresses
            first and bills the compressed size) — not per-second like compute.
            Representative rates run ~$23/TB (Capacity) to ~$40/TB (On Demand)
            and vary by region; see the pricing guide for your exact rate.
          </li>
          <li>
            <span className="font-medium">
              Billed storage = active + Time Travel + Fail-safe + diverged
              clones + staged files.
            </span>{" "}
            The “Total Storage” card above counts active table bytes; the rest
            is largely invisible history.
          </li>
          <li>
            <span className="font-medium">
              Time Travel &amp; Fail-safe accrue per day.
            </span>{" "}
            A permanent table carries 7–97 days of change history (0–90 d Time
            Travel + 7 d Fail-safe). Only the changed portion is retained —
            except a dropped or truncated table keeps a full copy that is still
            billed.
          </li>
        </ol>
        <a
          href="https://docs.snowflake.com/en/user-guide/cost-understanding-data-storage"
          target="_blank"
          rel="noopener noreferrer"
          className="inline-block text-sm text-primary hover:underline"
        >
          Snowflake docs
        </a>
      </section>

      {/* Query Frequency Chart */}
      <QueryFrequencyChart
        data={queryFrequency?.data ?? []}
        loading={freqLoading}
      />

      {/* Warehouse Utilization Table */}
      <div className="bg-card rounded-lg border p-4">
        <h2 className="text-lg font-semibold mb-4">
          Warehouse Utilization (7 days)
        </h2>
        {utilLoading ? (
          <div className="h-48 animate-pulse rounded bg-muted" />
        ) : warehouseUtil?.data?.length === 0 ? (
          <NoDataAvailable
            message="No warehouse activity"
            description="No warehouses ran queries in the last 7 days."
          />
        ) : (
          <MTable
            data={warehouseUtil?.data ?? []}
            columns={tableColumns}
          />
        )}
      </div>
    </div>
  );
}
