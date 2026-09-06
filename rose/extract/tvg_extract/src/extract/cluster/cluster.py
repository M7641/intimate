"""A2 — embed → cluster → categorical feature (README).

Clusters RAW description embeddings — no generation anywhere — and emits the
cluster id as a categorical feature. The expensive naming problem shrinks to
once-per-cluster: each cluster carries its medoid description (the exemplar
nearest the centroid) and its most distinctive terms, so a human or a single
LLM call can name it. This is `condense`'s move applied one stage earlier,
before any per-product generation.

Kept separate from the other A-paths on purpose: the composable surface is
``cluster_frame`` (frame in, frame + ``cluster_id`` out, plus a clusters
frame), which later patterns can mix with retrieval (A3) or spans (A4).
"""

from __future__ import annotations

import math
import re
from collections import Counter, defaultdict
from collections.abc import Callable

import polars as pl
import torch

from extract.dedup import description_key, normalise

DEFAULT_THRESHOLD = 0.65

TOKEN = re.compile(r"[a-z]{3,}")


def greedy_clusters(
    vectors: torch.Tensor, threshold: float = DEFAULT_THRESHOLD
) -> tuple[list[int], torch.Tensor]:
    """Single-pass clustering by cosine to a running centroid.

    Each unit vector joins the most similar existing cluster at or above
    ``threshold`` (updating its running-mean centroid), else seeds a new one —
    the same greedy move as ``discover.cluster``. Returns one cluster id per
    row (ids in first-seen order) and the final unit centroids ``(k, dim)``.
    """
    sums: list[torch.Tensor] = []
    assignment: list[int] = []
    for v in vectors:
        if sums:
            centroids = unit_rows(torch.stack(sums))
            sims = centroids @ v
            best = int(sims.argmax())
            if float(sims[best]) >= threshold:
                sums[best] = sums[best] + v
                assignment.append(best)
                continue
        sums.append(v.clone())
        assignment.append(len(sums) - 1)
    return assignment, unit_rows(torch.stack(sums))


def unit_rows(matrix: torch.Tensor) -> torch.Tensor:
    return matrix / matrix.norm(dim=1, keepdim=True).clamp(min=1e-9)


def exemplars(
    vectors: torch.Tensor, assignment: list[int], centroids: torch.Tensor
) -> dict[int, int]:
    """The medoid row per cluster: the member closest to its centroid.

    That row's description is the cluster's exemplar — the single text to show
    a human or an LLM when naming the cluster once.
    """
    best: dict[int, tuple[float, int]] = {}
    for row, cid in enumerate(assignment):
        sim = float(vectors[row] @ centroids[cid])
        if cid not in best or sim > best[cid][0]:
            best[cid] = (sim, row)
    return {cid: row for cid, (_, row) in best.items()}


def distinctive_terms(
    texts: list[str], assignment: list[int], k: int = 5
) -> dict[int, list[str]]:
    """Top-``k`` TF-IDF terms per cluster — the second naming aid.

    Term frequency within the cluster, damped by how many clusters the term
    appears in: generic catalogue words ("product", "available") score low,
    cluster-specific nouns score high. Pure python, no dependency.
    """
    term_counts: dict[int, Counter] = defaultdict(Counter)
    for text, cid in zip(texts, assignment):
        term_counts[cid].update(TOKEN.findall(text.lower()))
    n_clusters = len(term_counts)
    cluster_freq = Counter(term for counts in term_counts.values() for term in counts)
    top: dict[int, list[str]] = {}
    for cid, counts in term_counts.items():
        scored = {
            term: tf * math.log((1 + n_clusters) / cluster_freq[term])
            for term, tf in counts.items()
        }
        top[cid] = sorted(scored, key=scored.get, reverse=True)[:k]
    return top


def assign_to_centroids(
    vectors: torch.Tensor, centroids: torch.Tensor
) -> tuple[list[int], list[float]]:
    """Nearest-centroid assignment for NEW products (the incremental path).

    Returns the centroid index and cosine similarity per row — the similarity
    doubles as a confidence signal for routing (D1).
    """
    sims = vectors @ centroids.T
    best = sims.argmax(dim=1)
    return best.tolist(), sims.gather(1, best.unsqueeze(1)).squeeze(1).tolist()


def cluster_frame(
    df: pl.DataFrame,
    encoder,
    *,
    description_col: str = "description",
    threshold: float = DEFAULT_THRESHOLD,
    batch_size: int = 64,
    terms_k: int = 5,
    on_progress: Callable[[int, int], None] | None = None,
) -> tuple[pl.DataFrame, pl.DataFrame]:
    """Cluster a frame's descriptions; the composable A2 surface.

    Encodes each DISTINCT normalised description once (Step-0 dedup),
    greedy-clusters the vectors, and broadcasts the cluster id to duplicate
    rows. Returns ``(df + cluster_id, clusters)`` where ``clusters`` has one
    row per cluster: id, size (in rows), exemplar description, and its
    distinctive terms — everything needed to name each cluster once.
    """
    if description_col not in df.columns:
        msg = f"Expected column missing: {description_col!r}"
        raise ValueError(msg)

    descriptions = [d or "" for d in df.get_column(description_col).to_list()]
    keys = [description_key(d) for d in descriptions]
    first: dict[str, int] = {}
    for i, key in enumerate(keys):
        first.setdefault(key, i)

    distinct = [normalise(descriptions[i]) for i in first.values()]
    vectors = encoder.encode_batched(
        distinct, batch_size=batch_size, on_progress=on_progress
    )
    assignment, centroids = greedy_clusters(vectors, threshold)

    by_key = dict(zip(first, assignment))
    ids = [by_key[key] for key in keys]
    out = df.with_columns(pl.Series("cluster_id", ids))

    medoids = exemplars(vectors, assignment, centroids)
    terms = distinctive_terms(distinct, assignment, k=terms_k)
    sizes = Counter(ids)
    originals = list(first.values())  # distinct row index → original df row
    clusters = pl.DataFrame(
        {
            "cluster_id": sorted(medoids),
            "size": [sizes[cid] for cid in sorted(medoids)],
            "exemplar": [
                descriptions[originals[medoids[cid]]] for cid in sorted(medoids)
            ],
            "terms": [terms[cid] for cid in sorted(medoids)],
        }
    )
    return out, clusters


def name_clusters(
    clusters: pl.DataFrame, label: Callable[[str], str | None]
) -> pl.DataFrame:
    """Name each cluster ONCE from its exemplar — thousands of calls at most.

    ``label`` maps an exemplar description to a name (typically the Stage-1
    extractor's ``product_type`` on the medoid text — one LLM call per
    cluster, not per product). Injected as a plain callable so the clustering
    concept stays independent of any model.
    """
    names = [label(e) for e in clusters.get_column("exemplar").to_list()]
    return clusters.with_columns(pl.Series("name", names, dtype=pl.Utf8))
