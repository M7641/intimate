import {
  createRootRoute,
  createRoute,
  createRouter,
  Navigate,
} from "@tanstack/react-router";
import {
  FaChartBar,
  FaTable,
  FaExclamationTriangle,
  FaDollarSign,
  FaTachometerAlt,
  FaRedoAlt,
  FaDatabase,
  FaCoins,
  FaProjectDiagram,
} from "react-icons/fa";

import DashboardLayout from "@/components/DashboardLayout";
import { Button } from "@/components/ui/button";

import {
  SnowflakeDashboardPage,
  ExpensiveQueriesPage,
  CostByTablePage,
  WarehouseUtilizationPage,
  FailedQueriesPage,
  RepeatedQueriesPage,
  StoragePage,
  CreditSpendPage,
  SnowflakeQueryPlansPage,
} from "@/pages";

// ── Route config types ───────────────────────────────────────────────

export interface RouteConfig {
  path: string;
  label: string;
  icon: React.ReactNode;
  component: () => React.JSX.Element;
}

// ── Route definitions ────────────────────────────────────────────────
//
// The Snowflake overview is the app's landing page ("/"); all other
// analytics live under "/warehouse/...".

export const snowflakeRoutes: RouteConfig[] = [
  { path: "/", label: "Overview", icon: <FaChartBar />, component: SnowflakeDashboardPage },
  { path: "/warehouse/storage", label: "Storage", icon: <FaDatabase />, component: StoragePage },
  { path: "/warehouse/credit-spend", label: "Credit Spend", icon: <FaCoins />, component: CreditSpendPage },
  { path: "/warehouse/expensive-queries", label: "Expensive Queries", icon: <FaDollarSign />, component: ExpensiveQueriesPage },
  { path: "/warehouse/cost-by-table", label: "Cost by Table", icon: <FaTable />, component: CostByTablePage },
  { path: "/warehouse/warehouse-utilization", label: "Warehouse Utilization", icon: <FaTachometerAlt />, component: WarehouseUtilizationPage },
  { path: "/warehouse/failed-queries", label: "Failed Queries", icon: <FaExclamationTriangle />, component: FailedQueriesPage },
  { path: "/warehouse/repeated-queries", label: "Repeated Queries", icon: <FaRedoAlt />, component: RepeatedQueriesPage },
  { path: "/warehouse/query-plans", label: "Query Plans", icon: <FaProjectDiagram />, component: SnowflakeQueryPlansPage },
];

// ── Router builder ───────────────────────────────────────────────────

export function buildRouter() {
  const RootRoute = createRootRoute({
    component: DashboardLayout,
    notFoundComponent: () => <Navigate to="/" />,
    errorComponent: ({ error, reset }) => (
      <div
        role="alert"
        className="m-4 rounded-lg border border-destructive/40 bg-destructive/5 p-5"
      >
        <p className="text-sm font-semibold text-foreground">
          Something went wrong loading this view
        </p>
        <p className="mt-1 text-sm text-muted-foreground">
          The query couldn’t be completed. This is usually transient.
        </p>
        <div className="mt-4 flex items-center gap-3">
          <Button size="sm" onClick={() => reset()}>
            Try again
          </Button>
        </div>
        {error?.message && (
          <details className="mt-3">
            <summary className="text-xs text-muted-foreground cursor-pointer">
              Technical details
            </summary>
            <pre className="mt-1 text-xs font-mono text-muted-foreground whitespace-pre-wrap break-words">
              {error.message}
            </pre>
          </details>
        )}
      </div>
    ),
  });

  const routeTree = RootRoute.addChildren(
    snowflakeRoutes.map((r) =>
      createRoute({
        getParentRoute: () => RootRoute,
        path: r.path,
        component: r.component,
      }),
    ),
  );

  return createRouter({ routeTree });
}
