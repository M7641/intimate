import { useState } from "react";
import type { EngineOutcome } from "../api";
import { PlanTree } from "./PlanTree";

function fmt(n: number | null): string {
  if (n === null) return "—";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return Number.isInteger(n) ? n.toString() : n.toFixed(2);
}

export function EnginePanel({ outcome }: { outcome: EngineOutcome }) {
  const [tab, setTab] = useState<"tree" | "raw">("tree");

  return (
    <section className="panel">
      <header className="panel__head">
        <div>
          <h2 className="panel__name">{outcome.name}</h2>
          <span className="panel__kind">{outcome.kind}</span>
        </div>
        {outcome.ok && outcome.plan && (
          <div className="panel__summary">
            {outcome.plan.summary.total_cost !== null && (
              <div className="stat">
                <span className="stat__value">{fmt(outcome.plan.summary.total_cost)}</span>
                <span className="stat__label">est. cost</span>
              </div>
            )}
            <div className="stat">
              <span className="stat__value">{fmt(outcome.plan.summary.est_rows)}</span>
              <span className="stat__label">est. rows</span>
            </div>
            {outcome.plan.summary.total_ms !== null && (
              <div className="stat">
                <span className="stat__value">{outcome.plan.summary.total_ms.toFixed(1)} ms</span>
                <span className="stat__label">measured</span>
              </div>
            )}
            <div className="stat">
              <span className="stat__value">{outcome.plan.summary.node_count}</span>
              <span className="stat__label">operators</span>
            </div>
          </div>
        )}
      </header>

      {!outcome.ok && (
        <div className="panel__error">
          <strong>This engine could not plan the query.</strong>
          <pre>{outcome.error}</pre>
        </div>
      )}

      {outcome.ok && outcome.plan && (
        <>
          {outcome.plan.notes.length > 0 && (
            <ul className="notes">
              {outcome.plan.notes.map((note, i) => (
                <li key={i} className={`note note--${note.level}`}>
                  {note.message}
                </li>
              ))}
            </ul>
          )}

          <div className="tabs">
            <button className={tab === "tree" ? "tab tab--on" : "tab"} onClick={() => setTab("tree")}>
              Plan tree
            </button>
            <button className={tab === "raw" ? "tab tab--on" : "tab"} onClick={() => setTab("raw")}>
              Raw EXPLAIN
            </button>
          </div>

          {tab === "tree" ? (
            <PlanTree root={outcome.plan.root} />
          ) : (
            <pre className="raw">{outcome.plan.raw}</pre>
          )}
        </>
      )}
    </section>
  );
}
