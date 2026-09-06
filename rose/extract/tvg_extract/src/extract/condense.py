"""Incremental condensing: open-text predictions → a nested concept taxonomy.

Stage 2 of the pipeline. Stage 1 (`extract hierarchy`) writes per-SKU *free
text* (a `product_type`) to the raw table. This collapses that text into a
compact tree of canonical **concepts**, and maps each SKU onto it.

It is built to be incremental and cheap — never a full refresh:

* Concept **centroids** are persisted (in the L2 table), so a run only embeds the
  *new* batch's distinct values and assigns them against existing centroids.
  History is never re-embedded or re-clustered.
* The levels are configurable via ``LEVELS`` (a single ``product_type`` by
  default). For a multi-level tree they nest: each level is only placed when
  its parent landed on an *active* concept.
* A non-matching value seeds a **pending** concept; it must accumulate
  ``min_support`` occurrences (across runs) before it is **promoted** to active.
  Until then the SKU is parked as ``"unallocated"`` at that level — one weird
  prediction can't spawn a junk concept.

The embedding (torch) is isolated in :func:`embed_values`; everything else is
pure Python over unit vectors, so the assignment/promotion logic is testable
without loading a model.
"""

import math

# The generated levels, top → bottom. A single value (product_type) is a flat
# taxonomy; add more for a nested tree (e.g. "segment", "family", "subtype").
LEVELS = ("product_type",)
UNALLOCATED = "unallocated"


def cosine(a, b):
    """Dot product — equals cosine because all vectors here are unit length."""
    return sum(x * y for x, y in zip(a, b))


def merge_centroid(centroid, support, vector):
    """Support-weighted running mean of unit vectors, renormalised to unit."""
    merged = [c * support + v for c, v in zip(centroid, vector)]
    norm = math.sqrt(sum(x * x for x in merged)) or 1.0
    return [x / norm for x in merged]


def condense_batch(concepts, rows, vectors, threshold=0.65, min_support=3, start_id=1):
    """Assign a batch of raw rows onto the concept tree, growing it as needed.

    ``concepts`` is the current L2 (list of concept dicts); ``rows`` the new raw
    rows (``id`` + a free-text column per level in ``LEVELS``); ``vectors`` a
    ``{text: unit-vector}`` map for every distinct value in the batch.

    Returns ``(concepts, mapped, stats)`` — the updated concept list (copies,
    inputs untouched), the L3 rows (canonical labels, ``"unallocated"`` where a
    level couldn't be placed), and a small stats dict.
    """
    concepts = [dict(c) for c in concepts]
    scope: dict[tuple, list[dict]] = {}
    next_id = start_id
    for c in concepts:
        scope.setdefault((c["level"], c["parent_id"]), []).append(c)
        next_id = max(next_id, c["concept_id"] + 1)

    stats = {"new_concepts": 0, "promoted": 0}

    def nearest(candidates, vector):
        best, best_sim = None, -1.0
        for c in candidates:
            sim = cosine(vector, c["centroid"])
            if sim > best_sim:
                best, best_sim = c, sim
        return best, best_sim

    def assign(level, parent_id, text):
        """Place one value; return its concept if active, else None (parked)."""
        nonlocal next_id
        vector = vectors.get(text)
        if not text or vector is None:
            return None
        here = scope.setdefault((level, parent_id), [])

        active = [c for c in here if c["status"] == "active"]
        match, sim = nearest(active, vector)
        if match is not None and sim >= threshold:
            match["centroid"] = merge_centroid(
                match["centroid"], match["support"], vector
            )
            match["support"] += 1
            return match

        pending = [c for c in here if c["status"] == "pending"]
        match, sim = nearest(pending, vector)
        if match is not None and sim >= threshold:
            match["centroid"] = merge_centroid(
                match["centroid"], match["support"], vector
            )
            match["support"] += 1
        else:
            match = {
                "concept_id": next_id,
                "level": level,
                "parent_id": parent_id,
                "canonical": text,
                "centroid": list(vector),
                "support": 1,
                "status": "pending",
            }
            next_id += 1
            here.append(match)
            concepts.append(match)
            stats["new_concepts"] += 1

        if match["status"] == "pending" and match["support"] >= min_support:
            match["status"] = "active"
            stats["promoted"] += 1
        return match if match["status"] == "active" else None

    mapped = []
    for row in rows:
        out = {"id": row.get("id")}
        parent_id, broken = None, False
        for level in LEVELS:
            concept = None if broken else assign(level, parent_id, row.get(level))
            out[level] = concept["canonical"] if concept else UNALLOCATED
            if concept:
                parent_id = concept["concept_id"]
            else:
                broken = True  # a level only places under an active parent
        mapped.append(out)

    stats["active"] = sum(1 for c in concepts if c["status"] == "active")
    stats["pending"] = sum(1 for c in concepts if c["status"] == "pending")
    return concepts, mapped, stats


def reconcile_batch(concepts, rows, vectors, threshold=0.65):
    """Re-place previously-unallocated rows against the CURRENT active concepts.

    Read-only: no new concepts, no support or centroid changes — it only checks
    whether promotions since a row was first mapped now let it land. ``rows`` are
    the unallocated rows carrying their *original raw text*. Returns updated
    ``(id, <level>)`` mappings; a level stays ``"unallocated"``
    if still unplaceable (and its children with it).
    """
    scope: dict[tuple, list[dict]] = {}
    for c in concepts:
        if c["status"] == "active":
            scope.setdefault((c["level"], c["parent_id"]), []).append(c)

    def place(level, parent_id, text):
        vector = vectors.get(text)
        if not text or vector is None:
            return None
        best, best_sim = None, -1.0
        for c in scope.get((level, parent_id), []):
            sim = cosine(vector, c["centroid"])
            if sim > best_sim:
                best, best_sim = c, sim
        return best if best is not None and best_sim >= threshold else None

    mapped = []
    for row in rows:
        out = {"id": row.get("id")}
        parent_id, broken = None, False
        for level in LEVELS:
            concept = None if broken else place(level, parent_id, row.get(level))
            out[level] = concept["canonical"] if concept else UNALLOCATED
            if concept:
                parent_id = concept["concept_id"]
            else:
                broken = True
        mapped.append(out)
    return mapped


def distinct_values(rows):
    """Every non-empty value across all levels in the batch (for embedding)."""
    values = set()
    for row in rows:
        for level in LEVELS:
            value = row.get(level)
            if value:
                values.add(value)
    return sorted(values)


def embed_values(values):
    """``{text: unit-vector(list[float])}`` for the given values (lazy torch)."""
    from extract.discover import embed

    values = list(values)
    if not values:
        return {}
    matrix = embed(values).tolist()
    return dict(zip(values, matrix))
