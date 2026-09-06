"""saga — context database for AI agents (the mechanical core, offline).

Four seams mark where production swaps in the real components:

    embed.py    deterministic hashing embedding   → prod: a real model
                                                      (OpenAI, BGE, nomic...)
    extract.py  rule-based fact extraction         → prod: an LLM call
    store.py    SQLite + FTS5 + vector column       → prod: pgvector, Qdrant,
                                                      LanceDB...
    retrieve.py vector+keyword+recency fusion (RRF) → same, but the fusion
                                                      algorithm is unchanged

`memory.py` assembles these seams into the API the agent sees: `remember()` /
`recall()` / `assemble_context()`.
"""

from saga.memory import Memory

__all__ = ["Memory"]
