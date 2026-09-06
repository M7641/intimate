export function formatNumber(n: number | null | undefined): string {
  if (n == null) return "\u2014";
  return n.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null) return "\u2014";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function formatTimestampShort(timestamp: string): string {
  try {
    const date = new Date(timestamp);
    return date.toLocaleDateString(undefined, {
      month: "short",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return timestamp;
  }
}

export function formatTimestamp(timestamp: string): string {
  try {
    const date = new Date(timestamp);
    return date.toLocaleString(undefined, {
      year: "numeric",
      month: "short",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return timestamp;
  }
}

export function formatTableName(table: string | null | undefined): string {
  // A controlled Select can ask us to render before its value matches an
  // option, so `selectedOption()` may be undefined mid-transition. Guard it:
  // calling `.replace` on undefined throws and tears the Select out of the DOM.
  if (!table) return "";
  return table.replace(/_/g, " ").replace(/\b\w/g, (l) => l.toUpperCase());
}

/**
 * Display label for a column name. Same Title-Case + de-underscore transform
 * as table names, kept as its own export so every surface (data explorer
 * headers, column-analysis selector, table-info column list) renders a given
 * column identically.
 */
export function formatColumnName(column: string): string {
  return formatTableName(column);
}

import { formatDistanceToNow } from "date-fns";

/** Human "2 hours ago" style label for a load timestamp. */
export function formatRelativeTime(
  timestamp: string | null | undefined,
): string {
  if (!timestamp) return "—";
  try {
    return formatDistanceToNow(new Date(timestamp), { addSuffix: true });
  } catch {
    return timestamp;
  }
}
