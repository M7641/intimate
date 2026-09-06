import { Link } from "@tanstack/react-router";
import {
  FaSearch,
  FaInfoCircle,
  FaChartBar,
  FaProjectDiagram,
  FaHeartbeat,
} from "react-icons/fa";
import { Skeleton } from "@/components/ui/skeleton";
import {
  useDataExplorerSchemas,
  useDataExplorerTables,
  useRecentLoads,
} from "@/pages/_shared/api";
import StatCard from "@/components/StatCard";
import { formatTableName, formatRelativeTime } from "@/lib/format";

interface NavCard {
  title: string;
  description: string;
  href: string;
  icon: React.ReactNode;
}

const dataViewCards: NavCard[] = [
  {
    title: "Catalogue",
    description:
      "Map tables and their relationships as an interactive graph.",
    href: "/catalogue",
    icon: <FaProjectDiagram className="w-6 h-6" />,
  },
  {
    title: "Schema Health",
    description:
      "Track table freshness, empty tables, and row-count changes.",
    href: "/schema-health",
    icon: <FaHeartbeat className="w-6 h-6" />,
  },
  {
    title: "Data Explorer",
    description:
      "Browse schemas, tables, and snapshots. Filter and export data.",
    href: "/data-explorer",
    icon: <FaSearch className="w-6 h-6" />,
  },
  {
    title: "Table Info",
    description:
      "View table metadata, column details, storage info, and row history.",
    href: "/table-info",
    icon: <FaInfoCircle className="w-6 h-6" />,
  },
  {
    title: "Column Analysis",
    description:
      "Analyze column statistics, value distributions, and data quality.",
    href: "/column-analysis",
    icon: <FaChartBar className="w-6 h-6" />,
  },
];

/** Placeholder rows matching the list layout while a section's data loads. */
function ListRowsSkeleton({ rows = 6 }: { rows?: number }) {
  return (
    <>
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="flex items-center justify-between px-3 py-2">
          <Skeleton className="h-4 w-40 rounded" />
          <Skeleton className="h-4 w-16 rounded" />
        </div>
      ))}
    </>
  );
}

/** One row of the schema overview: schema name + its table count. */
function SchemaRow({ schema }: { schema: string }) {
  const tables = useDataExplorerTables(schema);
  return (
    <div className="flex items-center justify-between px-3 py-2 text-sm">
      <span className="font-medium">{schema}</span>
      {tables.isPending ? (
        <Skeleton className="h-4 w-12 rounded" />
      ) : (
        <span className="text-muted-foreground tabular-nums">
          {`${tables.data?.length ?? 0} tables`}
        </span>
      )}
    </div>
  );
}

export default function HomePage() {
  const schemas = useDataExplorerSchemas();
  const defaultSchema = schemas.data?.includes("stage")
    ? "stage"
    : (schemas.data?.[0] ?? "");

  const tables = useDataExplorerTables(defaultSchema);
  const recent = useRecentLoads(defaultSchema, 6);

  const lastUpdated = recent.data?.[0]?.last_loaded ?? null;

  return (
    <div className="space-y-8 max-w-4xl">
      {/* At a glance */}
      <div className="grid grid-cols-2 sm:grid-cols-3 gap-4">
        <StatCard
          label="Schemas"
          value={String(schemas.data?.length ?? 0)}
          loading={schemas.isPending}
          sub="Available schemas"
        />
        <StatCard
          label="Tables"
          value={String(tables.data?.length ?? 0)}
          loading={tables.isPending}
          sub={defaultSchema ? `in ${defaultSchema}` : "in default schema"}
        />
        <StatCard
          label="Last updated"
          value={formatRelativeTime(lastUpdated)}
          loading={recent.isPending}
          sub="Most recent load"
        />
      </div>

      {/* Data Views — launchpad */}
      <section>
        <h2 className="text-lg font-semibold mb-3">Data Views</h2>
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {dataViewCards.map((card) => (
            <Link
              key={card.href}
              to={card.href}
              className="group relative rounded-lg border bg-card p-5 transition-all hover:shadow-md hover:border-primary/30 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              <div className="flex items-start gap-4">
                <div className="mt-0.5 text-muted-foreground transition-colors group-hover:text-primary">
                  {card.icon}
                </div>
                <div className="min-w-0">
                  <h3 className="font-semibold group-hover:text-primary transition-colors">
                    {card.title}
                  </h3>
                  <p className="text-sm text-muted-foreground mt-1">
                    {card.description}
                  </p>
                </div>
              </div>
            </Link>
          ))}
        </div>
      </section>

      {/* Recently updated + Schemas */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <section>
          <h2 className="text-lg font-semibold mb-3">Recently updated</h2>
          <div className="rounded-lg border bg-card divide-y">
            {recent.isPending ? (
              <ListRowsSkeleton rows={6} />
            ) : recent.data && recent.data.length > 0 ? (
              recent.data.map((r) => (
                <Link
                  key={`${r.schema}.${r.table_name}`}
                  to="/table-info"
                  search={{ schema: r.schema, table: r.table_name }}
                  className="flex items-center justify-between gap-3 px-3 py-2 text-sm transition-colors hover:bg-accent/50 focus-visible:outline-none focus-visible:bg-accent/50"
                >
                  <span className="truncate font-medium">
                    {formatTableName(r.table_name)}
                  </span>
                  <span className="shrink-0 text-muted-foreground">
                    {formatRelativeTime(r.last_loaded)}
                  </span>
                </Link>
              ))
            ) : (
              <div className="px-3 py-2 text-sm text-muted-foreground">
                No recent loads found.
              </div>
            )}
          </div>
        </section>

        <section>
          <h2 className="text-lg font-semibold mb-3">Schemas</h2>
          <div className="rounded-lg border bg-card divide-y">
            {schemas.isPending ? (
              <ListRowsSkeleton rows={5} />
            ) : schemas.data && schemas.data.length > 0 ? (
              schemas.data.map((s) => <SchemaRow key={s} schema={s} />)
            ) : (
              <div className="px-3 py-2 text-sm text-muted-foreground">
                No schemas found.
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
