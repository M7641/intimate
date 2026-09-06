import { useState, useMemo, useCallback } from "react";

import { useRecentQueries, useQueryPlan } from "@/hooks/useSnowflakeData";
import type { RecentQuery, QueryPlanNode } from "@/types/snowflake";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ChevronDown, ChevronRight, BookOpen, ListTree } from "lucide-react";
import { cn } from "@/lib/utils";
import {
  nodeTint,
  nodeBar,
  statusTextClass,
  queryTypeBadge,
  formatQueryType,
} from "@/lib/plan-style";

// ---------------------------------------------------------------------------
// Query Plan Guide
// ---------------------------------------------------------------------------

function QueryPlanGuide() {
  const [open, setOpen] = useState(false);

  return (
    <div className="rounded-xl border bg-muted/30">
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-3 p-4 text-left group"
      >
        <div className="shrink-0 w-8 h-8 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center">
          <BookOpen className="w-4 h-4 text-primary" />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-sm font-semibold text-foreground/90">
            Understanding Query Plans
          </p>
          <p className="text-xs text-muted-foreground mt-0.5">
            Learn how to read execution plans and identify bottlenecks
          </p>
        </div>
        {open ? (
          <ChevronDown className="w-4 h-4 text-muted-foreground shrink-0" />
        ) : (
          <ChevronRight className="w-4 h-4 text-muted-foreground shrink-0 group-hover:translate-x-0.5 transition-transform" />
        )}
      </button>

      {open && (
        <div className="px-4 pb-5 space-y-5 text-sm leading-relaxed border-t border-border pt-4">
          {/* What is a query plan */}
          <div>
            <h4 className="font-semibold text-foreground/90 mb-1.5 flex items-center gap-2">
              <span className="w-5 h-5 rounded-full bg-primary/15 text-primary text-[10px] font-bold flex items-center justify-center">
                1
              </span>
              What is a Query Plan?
            </h4>
            <p className="text-muted-foreground text-xs leading-relaxed">
              When Snowflake executes a query, it builds an <span className="text-foreground/80 font-medium">execution plan</span> — a
              tree of operations (operators) that describes the steps taken to produce the result. Each node
              represents one operation: scanning a table, filtering rows, joining datasets, sorting, or
              aggregating. Data flows from the leaf nodes (bottom) up to the root (top).
            </p>
          </div>

          {/* Understanding execution percentage */}
          <div>
            <h4 className="font-semibold text-foreground/90 mb-1.5 flex items-center gap-2">
              <span className="w-5 h-5 rounded-full bg-primary/15 text-primary text-[10px] font-bold flex items-center justify-center">
                2
              </span>
              Reading the Execution Percentage
            </h4>
            <p className="text-muted-foreground text-xs leading-relaxed mb-2">
              Unlike traditional cost-based estimates, Snowflake provides the <span className="text-foreground/80 font-medium">actual
              execution time breakdown</span> for each operator. The percentage shown on each node represents
              how much of the total query time was spent in that operation.
            </p>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
              <div className="rounded-lg border p-2.5 sev-tint-critical">
                <div className="flex items-center gap-2 mb-1">
                  <div className="w-3 h-3 rounded-sm sev-dot-critical" />
                  <span className="text-xs font-semibold sev-fg-critical">40%+</span>
                </div>
                <p className="text-[11px] text-muted-foreground">
                  Critical hotspot — this operator dominates execution time.
                  Investigate immediately.
                </p>
              </div>
              <div className="rounded-lg border p-2.5 sev-tint-warning">
                <div className="flex items-center gap-2 mb-1">
                  <div className="w-3 h-3 rounded-sm sev-dot-warning" />
                  <span className="text-xs font-semibold sev-fg-warning">20–39%</span>
                </div>
                <p className="text-[11px] text-muted-foreground">
                  Significant cost — worth reviewing for optimisation
                  opportunities.
                </p>
              </div>
              <div className="rounded-lg border p-2.5 sev-tint-info">
                <div className="flex items-center gap-2 mb-1">
                  <div className="w-3 h-3 rounded-sm sev-dot-info" />
                  <span className="text-xs font-semibold sev-fg-info">5–19%</span>
                </div>
                <p className="text-[11px] text-muted-foreground">
                  Moderate cost — normal for most operations unless unexpectedly
                  high.
                </p>
              </div>
              <div className="rounded-lg border p-2.5 sev-tint-neutral">
                <div className="flex items-center gap-2 mb-1">
                  <div className="w-3 h-3 rounded-sm sev-dot-neutral" />
                  <span className="text-xs font-semibold sev-fg-neutral">&lt;5%</span>
                </div>
                <p className="text-[11px] text-muted-foreground">
                  Lightweight — minimal impact on overall performance.
                </p>
              </div>
            </div>
          </div>

          {/* Common operators */}
          <div>
            <h4 className="font-semibold text-foreground/90 mb-1.5 flex items-center gap-2">
              <span className="w-5 h-5 rounded-full bg-primary/15 text-primary text-[10px] font-bold flex items-center justify-center">
                3
              </span>
              Common Operators
            </h4>
            <div className="space-y-1.5 text-xs text-muted-foreground">
              <div className="flex gap-2">
                <span className="shrink-0 font-mono text-foreground/70 w-28">TableScan</span>
                <span>Reads data from a table. High cost here suggests adding clustering keys or reducing the data scanned with filters.</span>
              </div>
              <div className="flex gap-2">
                <span className="shrink-0 font-mono text-foreground/70 w-28">Filter</span>
                <span>Removes rows that don't match a condition. Usually lightweight unless applied to a massive dataset.</span>
              </div>
              <div className="flex gap-2">
                <span className="shrink-0 font-mono text-foreground/70 w-28">JoinFilter</span>
                <span>Combines two datasets. Expensive joins may indicate missing clustering or an inefficient join order.</span>
              </div>
              <div className="flex gap-2">
                <span className="shrink-0 font-mono text-foreground/70 w-28">Aggregate</span>
                <span>Groups and aggregates data (GROUP BY, COUNT, SUM). Costly when cardinality is very high.</span>
              </div>
              <div className="flex gap-2">
                <span className="shrink-0 font-mono text-foreground/70 w-28">Sort</span>
                <span>Orders result rows. Large sorts on unpartitioned data can be expensive.</span>
              </div>
            </div>
          </div>

          {/* Tips */}
          <div className="rounded-lg border p-3 bg-muted/30">
            <h4 className="font-semibold sev-fg-success text-xs mb-1.5">Quick Optimisation Tips</h4>
            <ul className="text-[11px] text-muted-foreground space-y-1 list-disc list-inside">
              <li>If <span className="font-mono text-foreground/70">TableScan</span> is the bottleneck, check your clustering keys and try to push filters as early as possible.</li>
              <li>High <span className="font-mono text-foreground/70">Rows</span> counts flowing through the plan often mean opportunities to filter earlier.</li>
              <li>Look for nodes where row count drops dramatically — that's where the most useful filtering happens.</li>
              <li>Compare execution time between similar queries to spot regressions.</li>
            </ul>
          </div>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatDuration(seconds: number): string {
  if (seconds < 1) return `${Math.round(seconds * 1000)}ms`;
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  return `${Math.floor(seconds / 60)}m ${Math.round(seconds % 60)}s`;
}

/** Safely parse a JSON variant column from Snowflake */
function parseVariant(val: string | null | undefined): Record<string, unknown> | null {
  if (!val) return null;
  try {
    const parsed = JSON.parse(val);
    return typeof parsed === "object" && parsed !== null ? parsed : null;
  } catch {
    return null;
  }
}

function parseParentOps(val: string | null | undefined): number[] {
  if (!val) return [];
  try {
    const parsed = JSON.parse(val);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

// ---------------------------------------------------------------------------
// Statistics extraction
// ---------------------------------------------------------------------------

/** Read a nested numeric value like `stats.io.bytes_scanned`, or null. */
function readNum(obj: Record<string, unknown> | null, ...path: string[]): number | null {
  let cur: unknown = obj;
  for (const key of path) {
    if (typeof cur !== "object" || cur === null) return null;
    cur = (cur as Record<string, unknown>)[key];
  }
  return typeof cur === "number" ? cur : null;
}

/** The Snowflake operator statistics we surface, flattened and typed. */
interface PlanStats {
  execPct: number;
  inputRows: number | null;
  outputRows: number | null;
  partitionsScanned: number | null;
  partitionsTotal: number | null;
  bytesScanned: number | null;
  cachePct: number | null;
  spilledLocal: number;
  spilledRemote: number;
  /** Per-category time split, only the categories that carry weight. */
  timeBreakdown: { label: string; pct: number }[];
}

/** The categories inside EXECUTION_TIME_BREAKDOWN, with display labels. */
const TIME_CATEGORIES: [key: string, label: string][] = [
  ["processing", "Processing"],
  ["local_disk_io", "Local I/O"],
  ["remote_disk_io", "Remote I/O"],
  ["synchronization", "Sync"],
  ["initialization", "Init"],
  ["network_communication", "Network"],
];

function extractStats(node: QueryPlanNode): PlanStats {
  const stats = parseVariant(node.operator_statistics);
  const time = parseVariant(node.execution_time_breakdown);

  const timeBreakdown = TIME_CATEGORIES.map(([key, label]) => ({
    label,
    pct: readNum(time, key) ?? 0,
  })).filter((c) => c.pct > 0);

  return {
    execPct: readNum(time, "overall_percentage") ?? 0,
    inputRows: readNum(stats, "input_rows"),
    outputRows: readNum(stats, "output_rows"),
    partitionsScanned: readNum(stats, "pruning", "partitions_scanned"),
    partitionsTotal: readNum(stats, "pruning", "partitions_total"),
    bytesScanned: readNum(stats, "io", "bytes_scanned"),
    cachePct: readNum(stats, "io", "percentage_scanned_from_cache"),
    spilledLocal: readNum(stats, "spilling", "bytes_spilled_local_storage") ?? 0,
    spilledRemote: readNum(stats, "spilling", "bytes_spilled_remote_storage") ?? 0,
    timeBreakdown,
  };
}

// ---------------------------------------------------------------------------
// Tree builder
// ---------------------------------------------------------------------------

interface PlanTreeNode {
  node: QueryPlanNode;
  children: PlanTreeNode[];
  stats: PlanStats;
}

interface PlanStep {
  stepId: number;
  roots: PlanTreeNode[];
}

/**
 * Build one tree per step. OPERATOR_ID restarts at 0 in each step and
 * PARENT_OPERATORS reference ids within the same step, so a single flat map
 * keyed by operator_id alone would collide across steps (the tell-tale symptom
 * being duplicate root nodes).
 */
function buildSteps(nodes: QueryPlanNode[]): PlanStep[] {
  const byStep = new Map<number, QueryPlanNode[]>();
  for (const n of nodes) {
    const list = byStep.get(n.step_id) ?? [];
    list.push(n);
    byStep.set(n.step_id, list);
  }

  const steps: PlanStep[] = [];
  for (const [stepId, stepNodes] of byStep) {
    const map = new Map<number, PlanTreeNode>();
    for (const n of stepNodes) {
      map.set(n.operator_id, { node: n, children: [], stats: extractStats(n) });
    }

    const roots: PlanTreeNode[] = [];
    for (const n of stepNodes) {
      const treeNode = map.get(n.operator_id)!;
      const parents = parseParentOps(n.parent_operators);
      if (parents.length === 0) {
        roots.push(treeNode);
      } else {
        for (const pid of parents) map.get(pid)?.children.push(treeNode);
      }
    }

    steps.push({
      stepId,
      roots: roots.length > 0 ? roots : [...map.values()],
    });
  }

  return steps.sort((a, b) => a.stepId - b.stepId);
}

/** Bytes → human string (KB/MB/GB). */
function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const val = bytes / Math.pow(1024, i);
  return `${val.toFixed(val < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

// ---------------------------------------------------------------------------
// Plan tree component
// ---------------------------------------------------------------------------

/** One labelled metric chip in the stats row. */
function Stat({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: "critical" | "warning";
}) {
  return (
    <span
      className={cn(
        tone === "critical" && "sev-fg-critical font-medium",
        tone === "warning" && "sev-fg-warning font-medium",
      )}
    >
      <span
        className={cn(
          "font-medium",
          tone ? "opacity-80" : "text-foreground/70",
        )}
      >
        {label}:
      </span>{" "}
      {value}
    </span>
  );
}

/** Segmented bar showing where a node's time went, by category. */
function TimeBreakdownBar({ segments }: { segments: { label: string; pct: number }[] }) {
  const total = segments.reduce((s, c) => s + c.pct, 0);
  if (total <= 0) return null;

  // Fixed hues per category keep the same colour meaning across nodes.
  const hue: Record<string, string> = {
    Processing: "bg-sky-500/70",
    "Local I/O": "bg-amber-500/70",
    "Remote I/O": "bg-rose-500/70",
    Sync: "bg-violet-500/70",
    Init: "bg-slate-400/70",
    Network: "bg-emerald-500/70",
  };

  return (
    <div className="mt-2">
      <div className="flex h-1.5 rounded-full overflow-hidden bg-foreground/10">
        {segments.map((s) => (
          <div
            key={s.label}
            className={cn("h-full", hue[s.label] ?? "bg-foreground/30")}
            style={{ width: `${(s.pct / total) * 100}%` }}
            title={`${s.label}: ${s.pct.toFixed(1)}%`}
          />
        ))}
      </div>
      <div className="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-[10px] text-muted-foreground">
        {segments.map((s) => (
          <span key={s.label} className="flex items-center gap-1">
            <span className={cn("w-2 h-2 rounded-sm", hue[s.label] ?? "bg-foreground/30")} />
            {s.label} {s.pct.toFixed(0)}%
          </span>
        ))}
      </div>
    </div>
  );
}

function PlanNode({ treeNode, depth = 0 }: { treeNode: PlanTreeNode; depth?: number }) {
  const { node, children, stats } = treeNode;
  const attrs = parseVariant(node.operator_attributes);
  const { execPct } = stats;

  // Partition pruning: the fraction of micro-partitions actually read. Reading
  // (nearly) all of them is Snowflake's clearest "no pruning happened" signal.
  const scannedAll =
    stats.partitionsScanned !== null &&
    stats.partitionsTotal !== null &&
    stats.partitionsTotal > 0 &&
    stats.partitionsScanned / stats.partitionsTotal >= 0.95;

  const spilled = stats.spilledLocal + stats.spilledRemote;

  return (
    <div className={cn("relative", depth > 0 && "ml-6 mt-2")}>
      {/* connector line */}
      {depth > 0 && (
        <div className="absolute -left-3 top-0 bottom-1/2 w-px bg-border" />
      )}
      {depth > 0 && (
        <div className="absolute -left-3 top-1/2 w-3 h-px bg-border" />
      )}

      <div
        className={cn(
          "rounded-lg border p-3 transition-shadow duration-200 hover:shadow-sm",
          nodeTint(execPct),
        )}
      >
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 min-w-0">
            <span className="shrink-0 text-xs font-mono text-muted-foreground">
              #{node.operator_id}
            </span>
            <span className="font-semibold text-sm truncate">
              {node.operator_type}
            </span>
          </div>
          {execPct > 0 && (
            <span className="shrink-0 text-xs font-mono tabular-nums font-medium">
              {execPct.toFixed(1)}%
            </span>
          )}
        </div>

        {/* per-category execution-time bar */}
        {stats.timeBreakdown.length > 0 ? (
          <TimeBreakdownBar segments={stats.timeBreakdown} />
        ) : execPct > 0 ? (
          <div className="mt-2 h-1.5 rounded-full bg-foreground/10 overflow-hidden">
            <div
              className={cn("h-full rounded-full transition-[width] duration-500", nodeBar(execPct))}
              style={{ width: `${Math.max(execPct, 2)}%` }}
            />
          </div>
        ) : null}

        {/* stats row */}
        <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
          {stats.inputRows !== null && stats.outputRows !== null ? (
            <Stat
              label="Rows"
              value={`${stats.inputRows.toLocaleString()} → ${stats.outputRows.toLocaleString()}`}
            />
          ) : stats.outputRows !== null ? (
            <Stat label="Rows" value={stats.outputRows.toLocaleString()} />
          ) : null}

          {stats.partitionsScanned !== null && stats.partitionsTotal !== null && (
            <Stat
              label="Partitions"
              value={`${stats.partitionsScanned.toLocaleString()} / ${stats.partitionsTotal.toLocaleString()}`}
              tone={scannedAll ? "warning" : undefined}
            />
          )}

          {stats.bytesScanned !== null && stats.bytesScanned > 0 && (
            <Stat label="Scanned" value={formatBytes(stats.bytesScanned)} />
          )}

          {stats.cachePct !== null && stats.cachePct > 0 && (
            <Stat label="Cache" value={`${stats.cachePct.toFixed(0)}%`} />
          )}

          {spilled > 0 && (
            <Stat label="Spilled" value={formatBytes(spilled)} tone="critical" />
          )}

          {!!attrs?.["table_name"] && (
            <Stat label="Table" value={String(attrs["table_name"])} />
          )}
        </div>
      </div>

      {children.length > 0 && (
        <div className="relative">
          {children.map((child) => (
            <PlanNode
              key={child.node.operator_id}
              treeNode={child}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Query list item
// ---------------------------------------------------------------------------

function QueryCard({
  query,
  selected,
  onSelect,
}: {
  query: RecentQuery;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      onClick={onSelect}
      className={cn(
        "w-full text-left p-3 rounded-lg border transition-colors duration-200 group focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        selected
          ? "bg-primary/10 border-primary/40 shadow-sm"
          : "bg-card/50 border-border/50 hover:border-border hover:bg-card/80",
      )}
    >
      <div className="flex items-center justify-between gap-2 mb-1.5">
        <span
          className={cn(
            "text-[10px] font-semibold px-1.5 py-0.5 rounded border uppercase tracking-wider",
            queryTypeBadge(query.query_type),
          )}
        >
          {formatQueryType(query.query_type)}
        </span>
        <span className={cn("text-[10px] font-medium", statusTextClass(query.execution_status))}>
          {query.execution_status}
        </span>
      </div>

      <p className="text-xs font-mono text-foreground/80 line-clamp-2 leading-relaxed mb-2">
        {query.query_text.slice(0, 120)}
      </p>

      <div className="flex items-center justify-between text-[10px] text-muted-foreground">
        <span>{query.user_name}</span>
        <div className="flex items-center gap-3">
          <span className="tabular-nums">{formatDuration(query.execution_seconds)}</span>
          <span className="tabular-nums">
            {new Date(query.start_time).toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
            })}
          </span>
        </div>
      </div>
    </button>
  );
}

// ---------------------------------------------------------------------------
// Main page
// ---------------------------------------------------------------------------

export default function SnowflakeQueryPlansPage() {
  const [days, setDays] = useState(7);
  const [limit, setLimit] = useState(50);
  const [selectedQueryId, setSelectedQueryId] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const { data: recentData, isLoading: queriesLoading } = useRecentQueries(days, limit);
  const { data: planData, isLoading: planLoading } = useQueryPlan(selectedQueryId);

  const queries = recentData?.data ?? [];

  const filtered = useMemo(() => {
    if (!search.trim()) return queries;
    const q = search.toLowerCase();
    return queries.filter(
      (r) =>
        r.query_text.toLowerCase().includes(q) ||
        r.user_name.toLowerCase().includes(q) ||
        r.query_type.toLowerCase().includes(q) ||
        r.warehouse_name?.toLowerCase().includes(q),
    );
  }, [queries, search]);

  const selectedQuery = useMemo(
    () => queries.find((q) => q.query_id === selectedQueryId) ?? null,
    [queries, selectedQueryId],
  );

  const planSteps = useMemo(
    () => (planData?.data ? buildSteps(planData.data) : []),
    [planData],
  );

  // A metadata/cloud-services query (e.g. INFORMATION_SCHEMA) runs without
  // warehouse compute, so its operators carry no execution statistics. Detect
  // that so we can explain the absence rather than show a bare step list.
  const hasCostData = useMemo(
    () =>
      planSteps.some((s) =>
        s.roots.some(function walk(n): boolean {
          return (
            n.stats.execPct > 0 ||
            n.stats.bytesScanned !== null ||
            n.children.some(walk)
          );
        }),
      ),
    [planSteps],
  );

  const handleSelect = useCallback((id: string) => {
    setSelectedQueryId((prev) => (prev === id ? null : id));
  }, []);

  return (
    <div className="flex flex-col h-[calc(100vh-1.5rem)] -m-6">
      {/* Header */}
      <div className="px-6 py-4 border-b bg-background">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-foreground">
              Query Plan Explorer
            </h1>
            <p className="text-sm text-muted-foreground mt-0.5">
              Browse recent queries and inspect their execution plans
            </p>
          </div>
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted-foreground">Show:</span>
              <Select value={limit.toString()} onValueChange={(v) => setLimit(Number(v))}>
                <SelectTrigger className="w-20 h-8 text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="25">25</SelectItem>
                  <SelectItem value="50">50</SelectItem>
                  <SelectItem value="100">100</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted-foreground">Range:</span>
              <Select value={days.toString()} onValueChange={(v) => setDays(Number(v))}>
                <SelectTrigger className="w-28 h-8 text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1">Last 1 day</SelectItem>
                  <SelectItem value="3">Last 3 days</SelectItem>
                  <SelectItem value="7">Last 7 days</SelectItem>
                  <SelectItem value="14">Last 14 days</SelectItem>
                  <SelectItem value="30">Last 30 days</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>
      </div>

      {/* Main split */}
      <div className="flex flex-1 min-h-0">
        {/* LEFT — Query list */}
        <div className="w-[380px] shrink-0 border-r flex flex-col bg-card/30">
          {/* Search */}
          <div className="p-3 border-b">
            <input
              type="text"
              placeholder="Search queries..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full h-8 px-3 text-sm rounded-md border bg-background/80 placeholder:text-muted-foreground/50 focus:outline-none focus:ring-1 focus:ring-primary/40"
            />
            <p className="text-[10px] text-muted-foreground mt-1.5 tabular-nums">
              {filtered.length} queries
            </p>
          </div>

          {/* List */}
          <div className="flex-1 overflow-y-auto p-3 space-y-2">
            {queriesLoading ? (
              Array.from({ length: 8 }).map((_, i) => (
                <div key={i} className="h-24 rounded-lg bg-muted/50 animate-pulse" />
              ))
            ) : filtered.length === 0 ? (
              <p className="text-center text-sm text-muted-foreground py-12">
                No queries found
              </p>
            ) : (
              filtered.map((q) => (
                <QueryCard
                  key={q.query_id}
                  query={q}
                  selected={selectedQueryId === q.query_id}
                  onSelect={() => handleSelect(q.query_id)}
                />
              ))
            )}
          </div>
        </div>

        {/* RIGHT — Detail + Plan */}
        <div className="flex-1 overflow-y-auto bg-background">
          {!selectedQuery ? (
            <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
              <div className="w-16 h-16 rounded-full bg-muted/30 flex items-center justify-center mb-4">
                <ListTree className="w-8 h-8 opacity-40" strokeWidth={1.5} />
              </div>
              <p className="text-sm font-medium">Select a query</p>
              <p className="text-xs mt-1">Choose a query from the left to view its execution plan</p>
            </div>
          ) : (
            <div className="p-6 space-y-6">
              {/* Query metadata */}
              <div className="rounded-xl border bg-card p-5 shadow-sm">
                <div className="flex items-center gap-3 mb-3">
                  <span
                    className={cn(
                      "text-xs font-semibold px-2 py-1 rounded border uppercase tracking-wider",
                      queryTypeBadge(selectedQuery.query_type),
                    )}
                  >
                    {formatQueryType(selectedQuery.query_type)}
                  </span>
                  <span className={cn("text-xs font-medium", statusTextClass(selectedQuery.execution_status))}>
                    {selectedQuery.execution_status}
                  </span>
                  <span className="text-xs text-muted-foreground font-mono ml-auto">
                    {selectedQuery.query_id}
                  </span>
                </div>

                <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 mb-4">
                  <div>
                    <p className="text-[10px] uppercase tracking-wider text-muted-foreground mb-0.5">
                      Duration
                    </p>
                    <p className="text-sm font-semibold tabular-nums">
                      {formatDuration(selectedQuery.execution_seconds)}
                    </p>
                  </div>
                  <div>
                    <p className="text-[10px] uppercase tracking-wider text-muted-foreground mb-0.5">
                      Data Scanned
                    </p>
                    <p className="text-sm font-semibold tabular-nums">
                      {selectedQuery.mb_scanned.toFixed(1)} MB
                    </p>
                  </div>
                  <div>
                    <p className="text-[10px] uppercase tracking-wider text-muted-foreground mb-0.5">
                      Warehouse
                    </p>
                    <p className="text-sm font-semibold truncate">
                      {selectedQuery.warehouse_name}
                    </p>
                  </div>
                  <div>
                    <p className="text-[10px] uppercase tracking-wider text-muted-foreground mb-0.5">
                      User
                    </p>
                    <p className="text-sm font-semibold truncate">
                      {selectedQuery.user_name}
                    </p>
                  </div>
                </div>

                {/* Query text */}
                <div className="rounded-lg border p-4 overflow-x-auto sql-surface">
                  <pre className="text-xs font-mono leading-relaxed whitespace-pre-wrap break-words">
                    {selectedQuery.query_text}
                  </pre>
                </div>
              </div>

              {/* Guide */}
              <QueryPlanGuide />

              {/* Query plan */}
              <div>
                <h2 className="text-lg font-semibold mb-3 flex items-center gap-2">
                  <span className="h-5 w-1 rounded-full bg-primary" />
                  Execution Plan
                </h2>

                {planLoading ? (
                  <div className="space-y-3">
                    {Array.from({ length: 4 }).map((_, i) => (
                      <div
                        key={i}
                        className="h-16 rounded-lg bg-muted/30 animate-pulse"
                        style={{ marginLeft: `${i * 24}px`, width: `${100 - i * 8}%` }}
                      />
                    ))}
                  </div>
                ) : planSteps.length === 0 ? (
                  <div className="rounded-lg border border-dashed p-8 text-center text-muted-foreground">
                    <p className="text-sm">No execution plan available for this query</p>
                    <p className="text-xs mt-1">
                      Plans are available for recently completed queries
                    </p>
                  </div>
                ) : (
                  <div className="space-y-5">
                    {!hasCostData && (
                      <div className="rounded-lg border p-3 text-xs leading-relaxed sev-tint-info">
                        This query ran in Snowflake's cloud-services layer without
                        warehouse compute (typical of{" "}
                        <span className="font-mono">INFORMATION_SCHEMA</span> and
                        other metadata queries), so its operators carry no
                        execution-cost statistics. The plan structure is shown, but
                        there is no per-operator cost to break down.
                      </div>
                    )}
                    {planSteps.map((step) => (
                      <div key={step.stepId} className="space-y-2">
                        {planSteps.length > 1 && (
                          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                            Step {step.stepId}
                          </p>
                        )}
                        {step.roots.map((root) => (
                          <PlanNode
                            key={`${step.stepId}-${root.node.operator_id}`}
                            treeNode={root}
                          />
                        ))}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
