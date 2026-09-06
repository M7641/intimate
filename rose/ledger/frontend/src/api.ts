// Mirrors the Rust API types in src/api.rs and src/plan.rs. Serde emits the
// Rust field names as-is, so these stay snake_case.

export interface PlanNode {
  id: number;
  operator: string;
  detail: string | null;
  est_rows: number | null;
  est_cost: number | null;
  actual_rows: number | null;
  actual_ms: number | null;
  hot: boolean;
  children: PlanNode[];
}

export interface Note {
  level: "info" | "warn";
  node_id: number | null;
  message: string;
}

export interface PlanSummary {
  total_cost: number | null;
  est_rows: number | null;
  node_count: number;
  total_ms: number | null;
}

export interface PlanResult {
  root: PlanNode;
  raw: string;
  summary: PlanSummary;
  notes: Note[];
}

export interface EngineOutcome {
  engine: string;
  name: string;
  kind: string;
  ok: boolean;
  error: string | null;
  plan: PlanResult | null;
}

export interface ExplainResponse {
  sql: string;
  analyzed: boolean;
  engines: EngineOutcome[];
}

export async function explain(sql: string, analyze: boolean): Promise<ExplainResponse> {
  const res = await fetch("/api/explain", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ sql, analyze }),
  });
  if (!res.ok) {
    throw new Error(`API error ${res.status}: ${await res.text()}`);
  }
  return res.json();
}

// Queries picked to make the row-store / columnar contrast visible.
export const EXAMPLES: { label: string; sql: string }[] = [
  {
    label: "Point lookup (favours the index)",
    sql: "SELECT * FROM orders WHERE customer_id = 4242",
  },
  {
    label: "Full-table aggregate (favours columnar)",
    sql: "SELECT status, count(*), avg(total)\nFROM orders\nGROUP BY status",
  },
  {
    label: "Join + group by (revenue per category)",
    sql: [
      "SELECT p.category, sum(oi.line_total) AS revenue",
      "FROM order_items oi",
      "JOIN products p ON p.id = oi.product_id",
      "GROUP BY p.category",
      "ORDER BY revenue DESC",
    ].join("\n"),
  },
  {
    label: "Selective range scan by date",
    sql: [
      "SELECT count(*)",
      "FROM orders",
      "WHERE order_date BETWEEN DATE '2023-06-01' AND DATE '2023-06-07'",
    ].join("\n"),
  },
];
