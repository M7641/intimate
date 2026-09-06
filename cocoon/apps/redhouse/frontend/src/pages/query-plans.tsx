import { useState, useMemo, useCallback } from "react";
import { useRecentQueries, useQueryPlan } from "@/hooks/useRedshiftData";
import type { RecentQuery, QueryPlanNode } from "@/types/redshift";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ChevronDown, ChevronRight, BookOpen, ListTree } from "lucide-react";
import { cn } from "@/lib/utils";
import { nodeTint, nodeBar } from "@/lib/plan-style";

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
            Learn how to read EXPLAIN output and understand cost estimates
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
              When Redshift receives a query, the planner creates an <span className="text-foreground/80 font-medium">execution plan</span> describing
              how to retrieve your data. You can see this plan using <code className="inline-code text-[11px]">EXPLAIN</code> before
              your query. The plan is a tree of operations — data flows from the leaves (scans) up to the root
              (final result). Redshift stores recent plans in <code className="inline-code text-[11px]">STL_EXPLAIN</code>.
            </p>
          </div>

          {/* Understanding cost */}
          <div>
            <h4 className="font-semibold text-foreground/90 mb-1.5 flex items-center gap-2">
              <span className="w-5 h-5 rounded-full bg-primary/15 text-primary text-[10px] font-bold flex items-center justify-center">
                2
              </span>
              Understanding Cost
            </h4>
            <p className="text-muted-foreground text-xs leading-relaxed mb-2">
              Each operator displays a cost in the format <code className="inline-code text-[11px]">cost=start..total</code>.
              These are <span className="text-foreground/80 font-medium">relative units</span> estimated by the query planner — not seconds or bytes.
            </p>
            <div className="rounded-lg bg-muted/40 border p-3 space-y-2 text-xs">
              <div className="flex items-start gap-3">
                <span className="shrink-0 font-mono text-foreground/80 font-semibold w-16">start</span>
                <span className="text-muted-foreground">
                  The estimated cost to begin producing the first row of output. A high start
                  cost means the operator must do significant work (e.g., sorting) before it can
                  output anything.
                </span>
              </div>
              <div className="flex items-start gap-3">
                <span className="shrink-0 font-mono text-foreground/80 font-semibold w-16">total</span>
                <span className="text-muted-foreground">
                  The estimated total cost to complete the entire operation and return all rows.
                  This is the primary number used to compare operators — <span className="text-foreground/70 font-medium">higher cost = more
                  work</span>.
                </span>
              </div>
              <div className="flex items-start gap-3">
                <span className="shrink-0 font-mono text-foreground/80 font-semibold w-16">rows</span>
                <span className="text-muted-foreground">
                  The planner's estimate of how many rows this operator will produce. Large
                  misestimates can cause suboptimal plan choices — run <code className="inline-code">ANALYZE</code> to
                  refresh table statistics.
                </span>
              </div>
            </div>
          </div>

          {/* Color legend */}
          <div>
            <h4 className="font-semibold text-foreground/90 mb-1.5 flex items-center gap-2">
              <span className="w-5 h-5 rounded-full bg-primary/15 text-primary text-[10px] font-bold flex items-center justify-center">
                3
              </span>
              Cost Color Scale
            </h4>
            <p className="text-muted-foreground text-xs leading-relaxed mb-2">
              Nodes are colored based on their total cost <span className="text-foreground/80 font-medium">relative to the most expensive
              node</span> in the plan:
            </p>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
              <div className="rounded-lg border p-2.5 sev-tint-critical">
                <div className="flex items-center gap-2 mb-1">
                  <div className="w-3 h-3 rounded-sm sev-dot-critical" />
                  <span className="text-xs font-semibold sev-fg-critical">40%+ of max cost</span>
                </div>
                <p className="text-[11px] text-muted-foreground">
                  Critical — this is the most expensive part of your query. Focus optimization here first.
                </p>
              </div>
              <div className="rounded-lg border p-2.5 sev-tint-warning">
                <div className="flex items-center gap-2 mb-1">
                  <div className="w-3 h-3 rounded-sm sev-dot-warning" />
                  <span className="text-xs font-semibold sev-fg-warning">20–39%</span>
                </div>
                <p className="text-[11px] text-muted-foreground">
                  Significant — worth reviewing, especially if it's a scan or nested loop.
                </p>
              </div>
              <div className="rounded-lg border p-2.5 sev-tint-info">
                <div className="flex items-center gap-2 mb-1">
                  <div className="w-3 h-3 rounded-sm sev-dot-info" />
                  <span className="text-xs font-semibold sev-fg-info">5–19%</span>
                </div>
                <p className="text-[11px] text-muted-foreground">
                  Moderate — typical for most operations in a well-tuned query.
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
                4
              </span>
              Common Operators
            </h4>
            <div className="space-y-1.5 text-xs text-muted-foreground">
              <div className="flex gap-2">
                <span className="shrink-0 font-mono text-foreground/70 w-28">Seq Scan</span>
                <span>Full table scan — reads every row. Consider adding sort keys or rewriting the query to filter earlier.</span>
              </div>
              <div className="flex gap-2">
                <span className="shrink-0 font-mono text-foreground/70 w-28">Hash Join</span>
                <span>Joins two tables by hashing one side. Efficient for large datasets. High cost may mean redistribution across nodes.</span>
              </div>
              <div className="flex gap-2">
                <span className="shrink-0 font-mono text-foreground/70 w-28">Nested Loop</span>
                <span>For each row in one table, scans the other. Very expensive for large datasets — usually signals a missing join key.</span>
              </div>
              <div className="flex gap-2">
                <span className="shrink-0 font-mono text-foreground/70 w-28">Sort</span>
                <span>Sorts rows for ORDER BY or merge joins. Costly on large unsorted data. Align sort keys with common ORDER BY clauses.</span>
              </div>
              <div className="flex gap-2">
                <span className="shrink-0 font-mono text-foreground/70 w-28">Aggregate</span>
                <span>Groups and computes aggregates. High cost with high cardinality groups (many distinct values).</span>
              </div>
              <div className="flex gap-2">
                <span className="shrink-0 font-mono text-foreground/70 w-28">XN Network</span>
                <span>Data redistribution across Redshift nodes. Frequent redistribution signals poor DISTKEY choices.</span>
              </div>
            </div>
          </div>

          {/* Broadcast joins & distribution */}
          <div>
            <h4 className="font-semibold text-foreground/90 mb-1.5 flex items-center gap-2">
              <span className="w-5 h-5 rounded-full bg-primary/15 text-primary text-[10px] font-bold flex items-center justify-center">
                5
              </span>
              Broadcast Joins &amp; Data Distribution
            </h4>
            <p className="text-muted-foreground text-xs leading-relaxed mb-3">
              Redshift is a <span className="text-foreground/80 font-medium">distributed system</span> — your data is split across
              multiple compute nodes. When two tables need to be joined, the data for both sides of the join
              must be on the <span className="text-foreground/80 font-medium">same node</span>. How Redshift achieves this has a
              major impact on performance.
            </p>

            <div className="space-y-3 mb-3">
              {/* Broadcast */}
              <div className="rounded-lg border p-3 sev-tint-warning">
                <div className="flex items-center gap-2 mb-1.5">
                  <span className="font-mono text-xs font-semibold sev-fg-warning">DS_BCAST_INNER</span>
                  <span className="text-[10px] text-muted-foreground font-medium">Broadcast Join</span>
                </div>
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  The entire inner (smaller) table is <span className="text-foreground/70 font-medium">copied to every node</span> in the
                  cluster. This works well when one table is small, but becomes extremely expensive as the
                  broadcasted table grows — the network cost scales with <code className="inline-code">nodes × rows</code>.
                  If you see this on a large table, it's a major red flag.
                </p>
              </div>

              {/* Redistribute */}
              <div className="rounded-lg border p-3 sev-tint-info">
                <div className="flex items-center gap-2 mb-1.5">
                  <span className="font-mono text-xs font-semibold sev-fg-info">DS_DIST_BOTH / DS_DIST_INNER</span>
                  <span className="text-[10px] text-muted-foreground font-medium">Redistribution</span>
                </div>
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  Rows are <span className="text-foreground/70 font-medium">hashed on the join key and shuffled</span> across
                  nodes so matching rows land together. <code className="inline-code">DS_DIST_BOTH</code> means
                  both tables are redistributed — the most expensive variant. <code className="inline-code">DS_DIST_INNER</code> means
                  only the inner table moves.
                </p>
              </div>

              {/* Co-located */}
              <div className="rounded-lg border p-3 sev-tint-success">
                <div className="flex items-center gap-2 mb-1.5">
                  <span className="font-mono text-xs font-semibold sev-fg-success">DS_DIST_NONE</span>
                  <span className="text-[10px] text-muted-foreground font-medium">Co-located Join</span>
                </div>
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  <span className="text-foreground/70 font-medium">No data movement needed</span> — both tables are already distributed on
                  the join key. This is the ideal scenario. It means both tables share the
                  same <code className="inline-code">DISTKEY</code> on the column being joined.
                </p>
              </div>
            </div>

            {/* How DISTKEY prevents broadcasts */}
            <div className="rounded-lg bg-muted/40 border p-3">
              <h5 className="text-xs font-semibold text-foreground/80 mb-1.5">How DISTKEY prevents broadcasts</h5>
              <p className="text-[11px] text-muted-foreground leading-relaxed mb-2">
                When you set <code className="inline-code">DISTKEY(column)</code> on a table,
                Redshift physically distributes rows across nodes using that column's hash value. If two tables
                that are frequently joined both have their DISTKEY set to the join column, matching rows are
                <span className="text-foreground/70 font-medium"> guaranteed to already be on the same node</span> — eliminating the need
                for any broadcast or redistribution.
              </p>
              <div className="rounded border p-2.5 font-mono text-[11px] leading-relaxed sql-surface">
                <div><span className="sev-fg-info">-- Before:</span> orders has no DISTKEY, customers DISTKEY(id)</div>
                <div><span className="sev-fg-warning">-- Plan shows DS_BCAST_INNER on orders</span></div>
                <div className="mt-1.5"><span className="sev-fg-success">-- After:</span> both tables DISTKEY on customer_id</div>
                <div><span className="sev-fg-success">-- Plan shows DS_DIST_NONE — no data movement!</span></div>
              </div>
            </div>
          </div>

          {/* Tips */}
          <div className="rounded-lg border p-3 bg-muted/30">
            <h4 className="font-semibold sev-fg-success text-xs mb-1.5">Quick Optimization Tips</h4>
            <ul className="text-[11px] text-muted-foreground space-y-1 list-disc list-inside">
              <li>Run <code className="inline-code">ANALYZE tablename</code> regularly — stale statistics cause the planner to pick bad plans.</li>
              <li>If <span className="font-mono text-foreground/70">Seq Scan</span> dominates cost, check whether a SORTKEY on the filtered column would help.</li>
              <li>Watch for <span className="font-mono text-foreground/70">Nested Loop</span> on large tables — this is almost always a performance red flag.</li>
              <li>Choose DISTKEY on your most common join column to eliminate <span className="font-mono text-foreground/70">DS_BCAST_INNER</span> and achieve <span className="font-mono text-foreground/70">DS_DIST_NONE</span> (co-located joins).</li>
              <li>If you see <span className="font-mono text-foreground/70">DS_BCAST_INNER</span> on a large table, that table is being copied to every node — set a matching DISTKEY to fix it.</li>
              <li>Compare estimated <span className="font-mono text-foreground/70">rows</span> to actual row counts — large discrepancies mean stale stats.</li>
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

/** Extract cost from plan_node text like "XN Hash Join (cost=0.00..5.87 rows=1 width=114)" */
function parsePlanCost(planNode: string): { cost: number | null; rows: number | null } {
  const costMatch = planNode.match(/cost=[\d.]+\.\.([\d.]+)/);
  const rowsMatch = planNode.match(/rows=(\d+)/);
  return {
    cost: costMatch ? parseFloat(costMatch[1]) : null,
    rows: rowsMatch ? parseInt(rowsMatch[1], 10) : null,
  };
}

/** Extract operator name from plan_node (before the parentheses) */
function parseOperatorName(planNode: string): string {
  // Strip leading arrow characters and whitespace
  const cleaned = planNode.replace(/^[\s->]+/, "");
  const parenIdx = cleaned.indexOf("(");
  return parenIdx > 0 ? cleaned.slice(0, parenIdx).trim() : cleaned.trim();
}

// ---------------------------------------------------------------------------
// Tree builder
// ---------------------------------------------------------------------------

interface PlanTreeNode {
  node: QueryPlanNode;
  children: PlanTreeNode[];
  operatorName: string;
  cost: number | null;
  rows: number | null;
  costPct: number;
}

function buildTree(nodes: QueryPlanNode[]): PlanTreeNode[] {
  if (nodes.length === 0) return [];

  // Find max cost for relative sizing
  let maxCost = 0;
  const parsed = nodes.map((n) => {
    const { cost, rows } = parsePlanCost(n.plan_node);
    if (cost !== null && cost > maxCost) maxCost = cost;
    return { node: n, operatorName: parseOperatorName(n.plan_node), cost, rows };
  });

  const map = new Map<number, PlanTreeNode>();
  for (const p of parsed) {
    const costPct = maxCost > 0 && p.cost !== null ? (p.cost / maxCost) * 100 : 0;
    map.set(p.node.node_id, {
      node: p.node,
      children: [],
      operatorName: p.operatorName,
      cost: p.cost,
      rows: p.rows,
      costPct,
    });
  }

  const roots: PlanTreeNode[] = [];
  for (const p of parsed) {
    const treeNode = map.get(p.node.node_id)!;
    const parent = map.get(p.node.parent_id);
    if (!parent || p.node.parent_id === 0) {
      roots.push(treeNode);
    } else {
      parent.children.push(treeNode);
    }
  }

  return roots.length > 0 ? roots : [...map.values()];
}

// ---------------------------------------------------------------------------
// Plan tree component
// ---------------------------------------------------------------------------

function PlanNode({ treeNode, depth = 0 }: { treeNode: PlanTreeNode; depth?: number }) {
  const { node, children, operatorName, cost, rows, costPct } = treeNode;

  return (
    <div className={cn("relative", depth > 0 && "ml-6 mt-2")}>
      {depth > 0 && (
        <div className="absolute -left-3 top-0 bottom-1/2 w-px bg-border" />
      )}
      {depth > 0 && (
        <div className="absolute -left-3 top-1/2 w-3 h-px bg-border" />
      )}

      <div
        className={cn(
          "rounded-lg border p-3 transition-shadow duration-200 hover:shadow-sm",
          nodeTint(costPct),
        )}
      >
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 min-w-0">
            <span className="shrink-0 text-xs font-mono text-muted-foreground">
              #{node.node_id}
            </span>
            <span className="font-semibold text-sm truncate">{operatorName}</span>
          </div>
          {cost !== null && (
            <span className="shrink-0 text-xs font-mono tabular-nums font-medium">
              cost: {cost.toFixed(2)}
            </span>
          )}
        </div>

        {/* cost bar */}
        {costPct > 0 && (
          <div className="mt-2 h-1.5 rounded-full bg-foreground/10 overflow-hidden">
            <div
              className={cn("h-full rounded-full transition-[width] duration-500", nodeBar(costPct))}
              style={{ width: `${Math.max(costPct, 2)}%` }}
            />
          </div>
        )}

        <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
          {rows !== null && (
            <span>
              <span className="text-foreground/70 font-medium">Est. Rows:</span>{" "}
              {rows.toLocaleString()}
            </span>
          )}
          {node.info && (
            <span className="truncate max-w-[400px]" title={node.info}>
              {node.info}
            </span>
          )}
        </div>
      </div>

      {children.length > 0 && (
        <div className="relative">
          {children.map((child) => (
            <PlanNode key={child.node.node_id} treeNode={child} depth={depth + 1} />
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
  const isAborted = query.aborted === 1;

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
        <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded border uppercase tracking-wider sev-badge-info">
          SQL
        </span>
        <span
          className={cn(
            "text-[10px] font-medium",
            isAborted ? "sev-fg-critical" : "sev-fg-success",
          )}
        >
          {isAborted ? "ABORTED" : "SUCCESS"}
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

export default function RedshiftQueryPlansPage() {
  const [days, setDays] = useState(7);
  const [limit, setLimit] = useState(50);
  const [selectedQueryId, setSelectedQueryId] = useState<number | null>(null);
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
        r.user_name?.toLowerCase().includes(q),
    );
  }, [queries, search]);

  const selectedQuery = useMemo(
    () => queries.find((q) => q.query_id === selectedQueryId) ?? null,
    [queries, selectedQueryId],
  );

  const planTree = useMemo(
    () => (planData?.data ? buildTree(planData.data) : []),
    [planData],
  );

  const handleSelect = useCallback((id: number) => {
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
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>
      </div>

      {/* Main split */}
      <div className="flex flex-1 min-h-0">
        {/* LEFT -- Query list */}
        <div className="w-[380px] shrink-0 border-r flex flex-col bg-card/30">
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

        {/* RIGHT -- Detail + Plan */}
        <div className="flex-1 overflow-y-auto bg-background">
          {!selectedQuery ? (
            <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
              <div className="w-16 h-16 rounded-full bg-muted/30 flex items-center justify-center mb-4">
                <ListTree className="w-8 h-8 opacity-40" strokeWidth={1.5} />
              </div>
              <p className="text-sm font-medium">Select a query</p>
              <p className="text-xs mt-1">
                Choose a query from the left to view its execution plan
              </p>
            </div>
          ) : (
            <div className="p-6 space-y-6">
              {/* Query metadata */}
              <div className="rounded-xl border bg-card p-5 shadow-sm">
                <div className="flex items-center gap-3 mb-3">
                  <span className="text-xs font-semibold px-2 py-1 rounded border uppercase tracking-wider sev-badge-info">
                    SQL
                  </span>
                  <span
                    className={cn(
                      "text-xs font-medium",
                      selectedQuery.aborted ? "sev-fg-critical" : "sev-fg-success",
                    )}
                  >
                    {selectedQuery.aborted ? "ABORTED" : "SUCCESS"}
                  </span>
                  <span className="text-xs text-muted-foreground font-mono ml-auto">
                    Query #{selectedQuery.query_id}
                  </span>
                </div>

                <div className="grid grid-cols-2 sm:grid-cols-3 gap-4 mb-4">
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
                      User
                    </p>
                    <p className="text-sm font-semibold truncate">
                      {selectedQuery.user_name}
                    </p>
                  </div>
                  <div>
                    <p className="text-[10px] uppercase tracking-wider text-muted-foreground mb-0.5">
                      Started
                    </p>
                    <p className="text-sm font-semibold tabular-nums">
                      {new Date(selectedQuery.start_time).toLocaleString()}
                    </p>
                  </div>
                </div>

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
                ) : planTree.length === 0 ? (
                  <div className="rounded-lg border border-dashed p-8 text-center text-muted-foreground">
                    <p className="text-sm">No execution plan available for this query</p>
                    <p className="text-xs mt-1">
                      Plans are available for recently completed queries in STL_EXPLAIN
                    </p>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {planTree.map((root) => (
                      <PlanNode key={root.node.node_id} treeNode={root} />
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
