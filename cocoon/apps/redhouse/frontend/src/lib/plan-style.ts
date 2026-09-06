/**
 * Shared styling for the Redshift and Snowflake query-plan explorers.
 *
 * Maps a relative cost/execution percentage (and query status/type) onto the
 * theme-aware severity classes defined in `assets/dataviz.css`. Returning whole
 * literal class strings keeps them detectable by Tailwind's scanner and keeps
 * the color logic in one place for both warehouses.
 */

/** Tint + border for a plan-node card, by share of total plan cost/time. */
export function nodeTint(pct: number): string {
  if (pct >= 40) return "sev-tint-critical";
  if (pct >= 20) return "sev-tint-warning";
  if (pct >= 5) return "sev-tint-info";
  return "sev-tint-neutral";
}

/** Solid fill for the execution/cost bar inside a plan node. */
export function nodeBar(pct: number): string {
  if (pct >= 40) return "sev-dot-critical";
  if (pct >= 20) return "sev-dot-warning";
  if (pct >= 5) return "sev-dot-info";
  return "sev-dot-neutral";
}

/** Text color for an execution status label. */
export function statusTextClass(status: string): string {
  const s = status.toUpperCase();
  if (s === "SUCCESS") return "sev-fg-success";
  if (s === "FAIL" || s === "INCIDENT" || s === "ABORTED") return "sev-fg-critical";
  return "sev-fg-warning";
}

/** Human-readable label for a raw query-type enum. */
export function formatQueryType(type: string): string {
  if (type === "CREATE_TABLE_AS_SELECT") return "CTAS";
  return type.replace(/_/g, " ");
}

/** Badge classes (tint + border + text) for a query type. */
export function queryTypeBadge(type: string): string {
  const map: Record<string, string> = {
    SELECT: "sev-badge-info",
    INSERT: "sev-badge-success",
    UPDATE: "sev-badge-warning",
    DELETE: "sev-badge-critical",
    MERGE: "sev-badge-info",
    CREATE_TABLE_AS_SELECT: "sev-badge-success",
  };
  return map[type] ?? "sev-badge-neutral";
}
