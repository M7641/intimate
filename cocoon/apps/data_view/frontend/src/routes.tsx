import {
  createRootRoute,
  createRoute,
  createRouter,
  Navigate,
} from "@tanstack/react-router";
import {
  FaSearch,
  FaInfoCircle,
  FaChartBar,
  FaHome,
  FaProjectDiagram,
  FaHeartbeat,
  FaBraille,
} from "react-icons/fa";

import DashboardLayout from "@/components/DashboardLayout";
import HomePage from "@/pages/home";
import DataExplorerPage from "@/pages/data-explorer";
import TableInfoPage from "@/pages/table-info";
import ColumnAnalysisPage from "@/pages/column-analysis";
import CataloguePage from "@/pages/catalogue";
import SchemaHealthPage from "@/pages/schema-health";
import GpuPlotPage from "@/pages/gpu-plot";

// ── Route config types ───────────────────────────────────────────────

export interface RouteConfig {
  path: string;
  label: string;
  icon: React.ReactNode;
  component: () => React.JSX.Element;
}

// ── Route definitions ────────────────────────────────────────────────

export const dataViewRoutes: RouteConfig[] = [
  { path: "/", label: "Home", icon: <FaHome />, component: HomePage },
  { path: "/catalogue", label: "Catalogue", icon: <FaProjectDiagram />, component: CataloguePage },
  { path: "/schema-health", label: "Schema Health", icon: <FaHeartbeat />, component: SchemaHealthPage },
  { path: "/data-explorer", label: "Data Explorer", icon: <FaSearch />, component: DataExplorerPage },
  { path: "/table-info", label: "Table Info", icon: <FaInfoCircle />, component: TableInfoPage },
  { path: "/column-analysis", label: "Column Analysis", icon: <FaChartBar />, component: ColumnAnalysisPage },
  { path: "/gpu-plot", label: "GPU Scatter", icon: <FaBraille />, component: GpuPlotPage },
];

// ── Router builder ───────────────────────────────────────────────────

export function buildRouter() {
  const RootRoute = createRootRoute({
    component: DashboardLayout,
    notFoundComponent: () => <Navigate to="/" />,
    errorComponent: ({ error }) => (
      <div
        role="alert"
        className="m-4 max-w-md rounded-lg border border-destructive/50 bg-destructive/10 p-4"
      >
        <p className="text-sm font-medium text-foreground">
          Something went wrong.
        </p>
        <p className="mt-1 text-sm text-muted-foreground">
          Try reloading the page. If this keeps happening, contact support.
        </p>
        <button
          type="button"
          onClick={() => window.location.reload()}
          className="mt-3 inline-flex h-8 items-center rounded-md border bg-background px-3 text-sm font-medium shadow-xs outline-none hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50"
        >
          Reload
        </button>
        <details className="mt-2">
          <summary className="cursor-pointer text-xs text-muted-foreground">
            Technical details
          </summary>
          <p className="mt-1 break-words font-mono text-xs text-muted-foreground">
            {error.message}
          </p>
        </details>
      </div>
    ),
  });

  const routeTree = RootRoute.addChildren(
    dataViewRoutes.map((r) =>
      createRoute({
        getParentRoute: () => RootRoute,
        path: r.path,
        component: r.component,
        // Preserve the explorer's selection in the URL (?schema=&table=&ts=):
        // home "recently updated" deep links seed it, and pages write it back.
        validateSearch: (search: Record<string, unknown>) =>
          search as {
            schema?: string;
            table?: string;
            ts?: string;
            column?: string;
          },
      }),
    ),
  );

  return createRouter({ routeTree });
}
