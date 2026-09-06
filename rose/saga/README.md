# saga

> The **mechanical core** of a _context database for AI agents_ (in the spirit of
> OpenViking / Letta / Zep-Graphiti / Mem0), stripped to the bone and running
> **offline**. A _stateless_ agent (an LLM call with no state) plugs in a memory
> that **crosses sessions**, **resolves contradictions over time**, and **chooses
> what to re-inject** into a limited context window.

Like the other pilots in `rose/` (galicia: S3→folder, Glue→SQLite), `saga`
**mimics production with local stand-ins** and clearly marks the _seams_ where
the real components plug in.

```
            ┌─────────────────────────────────────────────────────────┐
  message → │  remember()                                              │
            │    ├─ episodes  (RAW memory, per session)                │
            │    └─ extract() ─► facts (SEMANTIC memory, durable)       │
            │                     │  key (subject,predicate) single-val │
            │                     │  → contradiction = CLOSE the old one │
            └─────────────────────┼───────────────────────────────────┘
                                  ▼ valid_from / valid_to  (bi-temporal)
            ┌─────────────────────────────────────────────────────────┐
  query  → │  recall()  =  RRF( vector , keyword(FTS) , recency )       │  → facts
            │  assemble_context()  =  relevant facts + recent turns     │  → prompt
            └─────────────────────────────────────────────────────────┘     block
```

## The four seams (local stand-in → prod)

| Layer           | File          | Local stand-in (this pilot)                       | In production                                               |
| --------------- | ------------- | ------------------------------------------------- | ----------------------------------------------------------- |
| Embedding       | `embed.py`    | token hashing ("hashing trick"), deterministic    | a real encoder: OpenAI `text-embedding-3`, BGE, nomic-embed |
| Extraction      | `extract.py`  | regex + cardinality declared by hand              | **an LLM call** per message ("return the facts as JSON")    |
| Storage / index | `store.py`    | SQLite + FTS5 + vector BLOB column                | pgvector, Qdrant, LanceDB; the Zep/Graphiti _graph_         |
| Fusion          | `retrieve.py` | RRF(vector, FTS, recency) — **unchanged in prod** | identical: RRF is the real algorithm                        |

The point: the **algorithm** (bi-temporality, RRF, episodic/semantic split) is
the real product; the seams are just wiring. You can replace any one of them
without touching the rest.

## What it demonstrates (and why an agent needs it)

1. **Episodic vs semantic.** Raw turns are stored _and_ consolidated into durable
   facts. An agent cannot keep everything in its window; it needs both levels.
2. **Cross-session.** A _new_ session, with no chat history at all, still recovers
   the facts — that is what distinguishes a context database from a plain
   conversation history.
3. **Contradiction via bi-temporality.** "I moved" does not `UPDATE`: it **closes**
   the old fact (`valid_to`) and inserts the new one. Freshness becomes a property
   of _storage_, not a filter you can forget.
4. **Hybrid retrieval.** Vector (fuzzy semantics) + FTS (exact, rare entities) +
   recency, fused via **Reciprocal Rank Fusion** — robust without scale
   calibration.
5. **Time-travel.** "What did the agent know on monday?" is a query (`asof`), not
   log archaeology. Essential for audit and reproducibility.

## Run

```bash
cd rose/saga
uv run saga demo            # end-to-end narrative (throwaway temporary DB)
```

or without installing (the demo only needs the stdlib):

```bash
python3 -m saga.demo
```

## CLI

```bash
uv run saga remember --session monday "I'm Mike, based in Manchester."
uv run saga remember --session monday "Actually I moved to Lisbon."
uv run saga recall "user location"     # Lisbon (Manchester no longer surfaces)
uv run saga recall "user location" --asof 1   # time-travel: Manchester
uv run saga context --session monday "what do I like?"   # <memory> block to inject
uv run saga inspect                     # every fact, current AND stale
```

> Honest note on the embedding seam: the hashing embedding has **no** semantics —
> "located" and "location" are distinct tokens, so a query must share words with
> the fact to rank it well. A real encoder removes that constraint; this is
> exactly what `embed.py` gives up to stay offline. See `genisis.md`.

## Going further

`genisis.md` — how this category really _enables_ agents, how a data/ML team uses
it, when **not** to add one, and the adjacent work (entity graph, decay, RAG vs
memory, memory eval).
