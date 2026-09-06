"""Hybrid retrieval — vector + keyword + recency, fused via RRF.

This is the function that actually *enables* the agent: the context window is
small and expensive, so we cannot re-inject everything. Retrieval picks the
relevant subset.

Why *hybrid* and not vector-only:

  * the **vector** captures fuzzy semantic similarity ("where I live" ~
    "based in") but misses rare exact matches (a name, a SKU);
  * **keyword** (FTS/BM25) excels at exact and rare entities but is blind to
    synonyms;
  * **recency** breaks ties: at equal relevance, the most recent fact wins.

We combine them with **Reciprocal Rank Fusion**: each source votes `1/(k0+rank)`,
we sum. RRF ignores score scale (cosine ∈ [-1,1] vs BM25 ∈ ℝ⁺), which makes it
robust without calibration — hence its popularity in production.

Important: retrieval only operates on facts **valid** at the requested tick
(`store.facts_asof`). A stale fact can never surface. Freshness is therefore a
property of *storage*, not a filter we could forget here.
"""

from __future__ import annotations

from dataclasses import dataclass

from saga import embed
from saga.store import FactRow, Store

_K0 = 60  # standard RRF constant; dampens the weight of the very top ranks


@dataclass
class Scored:
    fact: FactRow
    score: float
    why: str  # readable trace: where the score came from (debug + audit)


def retrieve(
    store: Store, query: str, *, k: int = 5, asof: int | None = None
) -> list[Scored]:
    """Top-`k` facts relevant to `query`, seen at tick `asof` (None = now)."""
    facts = store.facts_asof(asof)
    if not facts:
        return []

    qv = embed.embed(query)

    # --- source 1: vector rank (descending cosine) ---
    by_vec = sorted(facts, key=lambda fv: embed.cosine(qv, fv[1]), reverse=True)
    vec_rank = {f.id: i for i, (f, _) in enumerate(by_vec)}

    # --- source 2: keyword rank (FTS) ---
    kw_rank = store.fts_match(query, asof=asof)

    # --- source 3: recency rank (descending valid_from) ---
    by_recency = sorted(facts, key=lambda fv: fv[0].valid_from, reverse=True)
    rec_rank = {f.id: i for i, (f, _) in enumerate(by_recency)}

    fact_by_id = {f.id: f for f, _ in facts}
    scored: list[Scored] = []
    for fid, fact in fact_by_id.items():
        s = 0.0
        parts: list[str] = []
        if fid in vec_rank:
            contrib = 1.0 / (_K0 + vec_rank[fid])
            s += contrib
            parts.append(f"vec#{vec_rank[fid]}")
        if fid in kw_rank:
            contrib = 1.0 / (_K0 + kw_rank[fid])
            s += contrib
            parts.append(f"kw#{kw_rank[fid]}")
        # recency is a half-vote: it breaks ties, it does not dominate.
        s += 0.5 / (_K0 + rec_rank[fid])
        parts.append(f"rec#{rec_rank[fid]}")
        scored.append(Scored(fact=fact, score=s, why=" ".join(parts)))

    scored.sort(key=lambda x: x.score, reverse=True)
    return scored[:k]
