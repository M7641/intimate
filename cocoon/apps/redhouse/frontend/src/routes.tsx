import {
  createRootRoute,
  createRoute,
  createRouter,
  Navigate,
} from "@tanstack/react-router";
import {
  FaChartBar,
  FaTable,
  FaClock,
  FaExclamationTriangle,
  FaArchive,
  FaUsers,
  FaCompressAlt,
  FaFilter,
  FaHdd,
  FaDatabase,
  FaProjectDiagram,
} from "react-icons/fa";

import DashboardLayout from "@/components/DashboardLayout";
import { Button } from "@/components/ui/button";

import {
  RedshiftDashboardPage,
  TableScansPage,
  QueryPerformancePage,
  SlowQueriesPage,
  UnusedTablesPage,
  RedshiftUserActivityPage,
  CompressionPage,
  FilterEffectivenessPage,
  DiskQueriesPage,
  StoragePage,
  RedshiftQueryPlansPage,
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
// The Redshift overview is the app's landing page ("/"); all other
// analytics live under "/warehouse/...".

export const redshiftRoutes: RouteConfig[] = [
  { path: "/", label: "Overview", icon: <FaChartBar />, component: RedshiftDashboardPage },
  { path: "/warehouse/storage", label: "Storage", icon: <FaDatabase />, component: StoragePage },
  { path: "/warehouse/table-scans", label: "Table Scans", icon: <FaTable />, component: TableScansPage },
  { path: "/warehouse/query-performance", label: "Query Performance", icon: <FaClock />, component: QueryPerformancePage },
  { path: "/warehouse/slow-queries", label: "Slow Queries", icon: <FaExclamationTriangle />, component: SlowQueriesPage },
  { path: "/warehouse/unused-tables", label: "Unused Tables", icon: <FaArchive />, component: UnusedTablesPage },
  { path: "/warehouse/user-activity", label: "User Activity", icon: <FaUsers />, component: RedshiftUserActivityPage },
  { path: "/warehouse/compression", label: "Compression", icon: <FaCompressAlt />, component: CompressionPage },
  { path: "/warehouse/filter-effectiveness", label: "Filter Effectiveness", icon: <FaFilter />, component: FilterEffectivenessPage },
  { path: "/warehouse/disk-queries", label: "Disk Queries", icon: <FaHdd />, component: DiskQueriesPage },
  { path: "/warehouse/query-plans", label: "Query Plans", icon: <FaProjectDiagram />, component: RedshiftQueryPlansPage },
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
    redshiftRoutes.map((r) =>
      createRoute({
        getParentRoute: () => RootRoute,
        path: r.path,
        component: r.component,
      }),
    ),
  );

  return createRouter({ routeTree });
}
