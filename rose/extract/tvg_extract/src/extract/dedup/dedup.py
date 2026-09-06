"""Step 0 — deduplicate before anything (README).

Retail catalogues are full of exact and near-duplicate descriptions
(size/colour variants, the same product from multiple sources). Extracting
each distinct description ONCE and broadcasting the result multiplies every
downstream approach for free. Three pieces:

- a normalised key, so trivially-different copies of a description share it;
- optional near-duplicate merging on unit-vector embeddings (greedy leader
  clustering, the same move as ``discover.cluster``);
- a persistent JSONL cache of results keyed per description, so repeat runs
  (e.g. a scheduled warehouse job) skip descriptions already extracted.
"""

from __future__ import annotations

import hashlib
import json
import logging
from collections.abc import Callable
from pathlib import Path

logger = logging.getLogger(__name__)


def normalise(text: str) -> str:
    """Collapse case and whitespace so trivially-different copies share a key."""
    return " ".join((text or "").lower().split())


def description_key(text: str) -> str:
    """Stable id for one description: sha256 of its normalised form."""
    return hashlib.sha256(normalise(text).encode("utf-8")).hexdigest()


def near_duplicate_leaders(vectors, threshold: float) -> list[int]:
    """Greedy leader clustering: the leader's index for each row.

    ``vectors`` are unit rows (dot product = cosine similarity), as produced by
    ``discover.embed``. Each row joins the most similar existing leader at or
    above ``threshold``, else becomes a leader itself — first occurrence wins,
    so a group's result comes from its earliest member.
    """
    leaders: list[int] = []
    assignment: list[int] = []
    for i in range(len(vectors)):
        if leaders:
            sims = vectors[leaders] @ vectors[i]
            best = int(sims.argmax())
            if float(sims[best]) >= threshold:
                assignment.append(leaders[best])
                continue
        leaders.append(i)
        assignment.append(i)
    return assignment


def merge_near_duplicates(
    descriptions: list[str],
    keys: list[str],
    threshold: float,
    embed: Callable | None = None,
) -> list[str]:
    """Remap each exact-duplicate key to its near-duplicate leader's key.

    Embeds one normalised text per distinct key (first occurrence) and
    leader-clusters them, so the caller extracts the leader once and
    broadcasts its result to the whole near-duplicate group. ``embed`` is
    injectable for tests; by default the lazy MiniLM encoder already used by
    ``discover``/``condense``.
    """
    first_text: dict[str, str] = {}
    for description, key in zip(descriptions, keys):
        first_text.setdefault(key, normalise(description))
    if len(first_text) < 2:
        return keys

    if embed is None:
        from extract.discover import embed

    distinct_keys = list(first_text)
    leaders = near_duplicate_leaders(embed(list(first_text.values())), threshold)
    remap = {key: distinct_keys[lead] for key, lead in zip(distinct_keys, leaders)}
    merged = sum(1 for key, lead in remap.items() if key != lead)
    if merged:
        logger.info(
            "near-dedup merged %d of %d distinct descriptions", merged, len(remap)
        )
    return [remap[key] for key in keys]


class ExtractionCache:
    """Append-only JSONL cache of extraction results, keyed per description.

    A cached result is only valid for the (model, schema) pair that produced
    it, so both are stored per entry and filtered on load — one file can hold
    several runs' worth without cross-contamination. Failed extractions (empty
    records) are never stored: a retry should re-attempt them.
    """

    def __init__(self, path: str | Path, *, model_id: str, schema_domain: str) -> None:
        self.path = Path(path)
        self.model_id = model_id
        self.schema_domain = schema_domain
        self.entries = self._load()

    def get(self, key: str) -> dict | None:
        return self.entries.get(key)

    def put_many(self, features_by_key: dict[str, dict]) -> None:
        """Append the successful, not-yet-cached entries and keep them in memory."""
        fresh = {
            key: feats
            for key, feats in features_by_key.items()
            if feats and key not in self.entries
        }
        if not fresh:
            return
        self.path.parent.mkdir(parents=True, exist_ok=True)
        with self.path.open("a", encoding="utf-8") as fh:
            for key, feats in fresh.items():
                row = {
                    "key": key,
                    "model": self.model_id,
                    "schema": self.schema_domain,
                    "features": feats,
                }
                fh.write(json.dumps(row, ensure_ascii=False) + "\n")
        self.entries.update(fresh)

    def _load(self) -> dict[str, dict]:
        if not self.path.exists():
            return {}
        entries: dict[str, dict] = {}
        for line in self.path.read_text(encoding="utf-8").splitlines():
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue  # a torn write must not poison the whole cache
            if (
                row.get("model") == self.model_id
                and row.get("schema") == self.schema_domain
            ):
                entries[row["key"]] = row["features"]
        logger.info("loaded %d cached extractions from %s", len(entries), self.path)
        return entries
