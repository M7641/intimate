import { Skeleton } from "@/components/ui/skeleton";
import { Button } from "@/components/ui/button";

type SkeletonVariant = "table" | "cards" | "chart" | "page";

function DataLoadError({
  error,
  onRetry,
}: {
  error: Error | null;
  onRetry?: () => void;
}) {
  if (!error) return null;

  // Always offer a recovery path: in-place refetch when the caller provides
  // one, otherwise a full reload. Customers should never hit a dead end.
  const retry = onRetry ?? (() => window.location.reload());

  return (
    <div
      role="alert"
      className="max-w-md rounded-lg border border-destructive/50 bg-destructive/10 p-4"
    >
      <p className="text-sm font-medium text-foreground">
        We couldn&apos;t load this data.
      </p>
      <p className="mt-1 text-sm text-muted-foreground">
        Check your connection and try again. If this keeps happening, contact
        support.
      </p>
      <Button variant="outline" size="sm" onClick={retry} className="mt-3">
        Try again
      </Button>
      <details className="mt-2">
        <summary className="cursor-pointer text-xs text-muted-foreground">
          Technical details
        </summary>
        <p className="mt-1 break-words font-mono text-xs text-muted-foreground">
          {error.message}
        </p>
      </details>
    </div>
  );
}

// ── Table skeleton ── matches MTable structure ──────────────────────

const TABLE_COL_COUNT = 6;
const TABLE_ROW_COUNT = 10;

function TableSkeleton() {
  return (
    <table className="min-w-full divide-y">
      <thead className="border-b-0">
        <tr>
          {Array.from({ length: TABLE_COL_COUNT }, (_, i) => (
            <th
              key={i}
              className="px-2 py-2 text-[12px] font-semibold tracking-wider"
            >
              <div className="flex items-center justify-center">
                <Skeleton className="h-3 w-16 rounded" />
              </div>
            </th>
          ))}
        </tr>
      </thead>
      <tbody className="divide-y">
        {Array.from({ length: TABLE_ROW_COUNT }, (_, rowIdx) => (
          <tr key={rowIdx} className="h-10">
            {Array.from({ length: TABLE_COL_COUNT }, (_, colIdx) => (
              <td
                key={colIdx}
                className="whitespace-nowrap text-[14px] text-center px-2"
              >
                <div className="flex items-center justify-center">
                  <Skeleton
                    className="h-3 rounded"
                    style={{
                      width: `${50 + ((rowIdx * 7 + colIdx * 13) % 5) * 10}%`,
                    }}
                  />
                </div>
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

// ── Cards skeleton ── matches StatCard grid ──────────────────────────

function CardsSkeleton() {
  return (
    <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
      {Array.from({ length: 4 }, (_, i) => (
        <div
          key={i}
          className="rounded-lg border bg-card p-4 min-w-0 space-y-2"
        >
          <Skeleton className="h-3 w-16 rounded" />
          <Skeleton className="h-6 w-20 rounded" />
          {i % 2 === 1 && <Skeleton className="h-3 w-12 rounded" />}
        </div>
      ))}
    </div>
  );
}

// ── Chart skeleton ── matches chart containers ──────────────────────

function ChartSkeleton() {
  return (
    <div className="rounded-lg border bg-card p-4 space-y-3">
      <Skeleton className="h-3.5 w-24 rounded" />
      <div className="flex items-end gap-2 h-[260px] pt-4 pb-8 px-4">
        {Array.from({ length: 12 }, (_, i) => (
          <Skeleton
            key={i}
            className="flex-1 rounded-t"
            style={{ height: `${25 + ((i * 37 + 13) % 60)}%` }}
          />
        ))}
      </div>
      <div className="flex justify-between px-4">
        {Array.from({ length: 6 }, (_, i) => (
          <Skeleton key={i} className="h-2.5 w-10 rounded" />
        ))}
      </div>
    </div>
  );
}

// ── Page skeleton ── matches table_info layout (cards + table + chart)

function PageSkeleton() {
  return (
    <div className="space-y-6">
      <CardsSkeleton />
      <div className="space-y-2">
        <Skeleton className="h-4 w-20 rounded" />
        <div className="rounded-lg border overflow-hidden">
          <TableSkeleton />
        </div>
      </div>
      <div className="space-y-2">
        <Skeleton className="h-4 w-36 rounded" />
        <ChartSkeleton />
      </div>
    </div>
  );
}

// ── Main component ──────────────────────────────────────────────────

const SKELETON_MAP: Record<SkeletonVariant, React.FC> = {
  table: TableSkeleton,
  cards: CardsSkeleton,
  chart: ChartSkeleton,
  page: PageSkeleton,
};

export default function DataLoading({
  isPending,
  error,
  onRetry,
  variant = "table",
}: {
  isPending: boolean;
  error?: Error | null;
  onRetry?: () => void;
  variant?: SkeletonVariant;
}) {
  if (!isPending && !error) {
    return null;
  }

  if (error) {
    return (
      <div className="flex justify-center items-center mt-4">
        <DataLoadError error={error} onRetry={onRetry} />
      </div>
    );
  }

  const SkeletonComponent = SKELETON_MAP[variant];
  return (
    <div aria-busy="true">
      <SkeletonComponent />
    </div>
  );
}
