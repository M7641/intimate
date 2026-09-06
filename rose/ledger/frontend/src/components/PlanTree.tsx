import type { PlanNode } from "../api";

function fmt(n: number | null): string {
  if (n === null) return "—";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return Number.isInteger(n) ? n.toString() : n.toFixed(2);
}

function NodeRow({ node, depth }: { node: PlanNode; depth: number }) {
  return (
    <>
      <div
        className={`node${node.hot ? " node--hot" : ""}`}
        style={{ marginLeft: depth * 20 }}
      >
        <div className="node__head">
          <span className="node__op">{node.operator}</span>
          {node.detail && <span className="node__detail">{node.detail}</span>}
          {node.hot && <span className="node__flag">bottleneck</span>}
        </div>
        <div className="node__metrics">
          {node.est_cost !== null && (
            <span className="metric" title="Estimated total cost (Postgres cost units)">
              cost {fmt(node.est_cost)}
            </span>
          )}
          <span className="metric" title="Estimated rows emitted">
            ~{fmt(node.est_rows)} rows
          </span>
          {node.actual_rows !== null && (
            <span className="metric metric--actual" title="Actual rows (ANALYZE)">
              {fmt(node.actual_rows)} actual
            </span>
          )}
          {node.actual_ms !== null && (
            <span className="metric metric--actual" title="Actual time in this operator (ANALYZE)">
              {node.actual_ms.toFixed(2)} ms
            </span>
          )}
        </div>
      </div>
      {node.children.map((child) => (
        <NodeRow key={child.id} node={child} depth={depth + 1} />
      ))}
    </>
  );
}

export function PlanTree({ root }: { root: PlanNode }) {
  return (
    <div className="tree">
      <NodeRow node={root} depth={0} />
    </div>
  );
}
