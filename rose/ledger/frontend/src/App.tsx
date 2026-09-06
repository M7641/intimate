import { useState } from "react";
import { explain, EXAMPLES, type ExplainResponse } from "./api";
import { EnginePanel } from "./components/EnginePanel";

export function App() {
  const [sql, setSql] = useState(EXAMPLES[0].sql);
  const [analyze, setAnalyze] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ExplainResponse | null>(null);

  async function run() {
    setLoading(true);
    setError(null);
    try {
      setResult(await explain(sql, analyze));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="app">
      <header className="app__header">
        <h1>
          ledger <span className="app__sub">query plan comparison</span>
        </h1>
        <p className="app__tagline">
          One query, planned by an analytical engine (DuckDB) and a transactional one
          (Postgres). See where each spends its cost.
        </p>
      </header>

      <div className="editor">
        <div className="editor__examples">
          {EXAMPLES.map((ex) => (
            <button key={ex.label} className="chip" onClick={() => setSql(ex.sql)}>
              {ex.label}
            </button>
          ))}
        </div>
        <textarea
          className="editor__sql"
          value={sql}
          onChange={(e) => setSql(e.target.value)}
          spellCheck={false}
          rows={6}
        />
        <div className="editor__actions">
          <label className="toggle" title="Actually execute the query to collect real timing">
            <input type="checkbox" checked={analyze} onChange={(e) => setAnalyze(e.target.checked)} />
            Analyze (execute for real timing)
          </label>
          <button className="run" onClick={run} disabled={loading || sql.trim() === ""}>
            {loading ? "Planning…" : "Explain"}
          </button>
        </div>
      </div>

      {error && <div className="app__error">{error}</div>}

      {result && (
        <div className="results">
          {result.engines.map((outcome) => (
            <EnginePanel key={outcome.engine} outcome={outcome} />
          ))}
        </div>
      )}
    </div>
  );
}
