import { LoaderCircle } from "lucide-react";

/**
 * Subtle "data is refreshing" affordance for an in-flight refetch, shown while
 * stale data stays on screen (the keepPreviousData case). This is deliberately
 * NOT the first-load skeleton — that one means "no data yet". See
 * docs/data-fetching-and-reactivity.md for the three loading states.
 *
 * Pass `when={query.isFetching && !!query.data}`.
 */
export default function FetchingIndicator({
  when,
  label,
  className,
}: {
  when: boolean;
  label?: string;
  className?: string;
}) {
  if (!when) return null;

  return (
    <span
      role="status"
      aria-live="polite"
      className={`flex items-center gap-1.5 text-xs text-muted-foreground ${className ?? ""}`}
    >
      <LoaderCircle className="size-3.5 animate-spin" />
      {label ?? "Updating…"}
    </span>
  );
}
