import { X } from "lucide-react";
import type {
  CatalogueEdge,
  EdgeColumn,
  LinkOverlap,
} from "@/pages/_shared/types";
import { cn } from "@/lib/utils";

function fmt(n: number): string {
  return n.toLocaleString();
}

function linkLabel(l: EdgeColumn): string {
  return l.source_column === l.target_column
    ? l.source_column
    : `${l.source_column} → ${l.target_column}`;
}

/** A horizontal bar showing how much of one side's keys are shared. */
function CoverageBar({ label, pct }: { label: string; pct: number }) {
  const width = `${Math.min(100, Math.max(0, pct)).toFixed(0)}%`;
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-xs text-muted-foreground">
        <span>{label}</span>
        <span className="font-mono text-foreground">{pct.toFixed(1)}%</span>
      </div>
      <div className="h-2 w-full overflow-hidden rounded-full bg-muted">
        <div className="h-full rounded-full bg-primary" style={{ width }} />
      </div>
    </div>
  );
}

/**
 * Side panel for a selected link: lets the user pick which column pair to
 * validate, then shows the measured value overlap as a confidence signal.
 */
export function OverlapPanel({
  edge,
  linkIndex,
  onPickLink,
  overlap,
  isPending,
  error,
  onClose,
}: {
  edge: CatalogueEdge;
  linkIndex: number;
  onPickLink: (i: number) => void;
  overlap: LinkOverlap | undefined;
  isPending: boolean;
  error: Error | null;
  onClose: () => void;
}) {
  // Verdict from the stronger of the two coverages.
  let verdict: string | null = null;
  if (overlap) {
    const best = Math.max(
      overlap.source_coverage_pct,
      overlap.target_coverage_pct,
    );
    if (overlap.overlap === 0) verdict = "No shared values — likely unrelated.";
    else if (best >= 90)
      verdict = "Strong match — behaves like a real foreign key.";
    else if (best >= 50) verdict = "Partial overlap.";
    else verdict = "Weak overlap.";
  }

  return (
    <aside className="w-80 shrink-0 overflow-auto border-l bg-card p-4 space-y-4">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-sm font-medium">
            <span className="truncate font-mono">{edge.source}</span>
            <span className="text-muted-foreground">→</span>
            <span className="truncate font-mono">{edge.target}</span>
          </div>
          <span
            className={cn(
              "mt-1 inline-block rounded-full px-2 py-0.5 text-xs",
              edge.kind === "declared" && "bg-primary/15 text-primary",
              edge.kind === "inferred" && "bg-muted text-muted-foreground",
            )}
          >
            {edge.kind === "declared"
              ? "declared foreign key"
              : "inferred from naming"}
          </span>
        </div>
        <button
          type="button"
          aria-label="Close"
          onClick={() => onClose()}
          className="rounded p-1 text-muted-foreground hover:bg-accent"
        >
          <X className="size-4" />
        </button>
      </div>

      {/* Column-pair picker (only meaningful when there is more than one) */}
      {edge.links.length > 1 && (
        <div className="space-y-1">
          <p className="text-xs text-muted-foreground">Column pair</p>
          <div className="flex flex-wrap gap-1.5">
            {edge.links.map((l, i) => (
              <button
                key={i}
                type="button"
                onClick={() => onPickLink(i)}
                className={cn(
                  "rounded-md border px-2 py-1 font-mono text-xs",
                  i === linkIndex
                    ? "bg-primary text-primary-foreground"
                    : "hover:bg-accent",
                )}
              >
                {linkLabel(l)}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Overlap measurement */}
      <div className="space-y-3">
        {isPending && (
          <p className="text-sm text-muted-foreground">Measuring overlap…</p>
        )}

        {error && (
          <p className="text-sm text-destructive">
            Couldn't measure overlap. {error.message}
          </p>
        )}

        {overlap && (
          <>
            <div className="grid grid-cols-3 gap-2 text-center">
              <div className="rounded-lg border p-2">
                <div className="text-lg font-semibold">
                  {fmt(overlap.source_distinct)}
                </div>
                <div className="text-xs text-muted-foreground">source keys</div>
              </div>
              <div className="rounded-lg border p-2">
                <div className="text-lg font-semibold">
                  {fmt(overlap.overlap)}
                </div>
                <div className="text-xs text-muted-foreground">shared</div>
              </div>
              <div className="rounded-lg border p-2">
                <div className="text-lg font-semibold">
                  {fmt(overlap.target_distinct)}
                </div>
                <div className="text-xs text-muted-foreground">target keys</div>
              </div>
            </div>

            <CoverageBar
              label="of source found in target"
              pct={overlap.source_coverage_pct}
            />
            <CoverageBar
              label="of target found in source"
              pct={overlap.target_coverage_pct}
            />

            <p className="text-sm text-foreground">{verdict}</p>
            <p className="text-xs text-muted-foreground">
              Measured on each table's latest snapshot.
            </p>
          </>
        )}
      </div>
    </aside>
  );
}
