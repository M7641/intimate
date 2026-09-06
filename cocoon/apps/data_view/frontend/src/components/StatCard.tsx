import InfoHint from "@/components/InfoHint";
import { Skeleton } from "@/components/ui/skeleton";

export default function StatCard({
  label,
  value,
  sub,
  hint,
  loading,
}: {
  label: string;
  value: string;
  sub?: string;
  /** Optional plain-language explanation, shown via a "?" affordance. */
  hint?: string;
  /** Render a skeleton in place of the value while the source query loads. */
  loading?: boolean;
}) {
  return (
    <div className="flex h-full flex-col rounded-lg border bg-card p-4 min-w-0">
      <p className="flex items-center gap-1 text-xs text-muted-foreground">
        <span className="truncate" title={label}>
          {label}
        </span>
        {hint && <InfoHint text={hint} className="shrink-0" />}
      </p>
      {loading ? (
        <Skeleton className="mt-1.5 mb-0.5 h-6 w-20 rounded" />
      ) : (
        <p
          className="text-xl font-semibold tracking-tight mt-1 tabular-nums truncate"
          title={value}
        >
          {value}
        </p>
      )}
      {/* Reserve the sub-line's row even when absent so a row of cards keeps a
          uniform baseline; combined with h-full the grid renders them flush. */}
      <p className="text-xs text-muted-foreground mt-0.5 tabular-nums truncate min-h-4">
        {sub ?? " "}
      </p>
    </div>
  );
}
