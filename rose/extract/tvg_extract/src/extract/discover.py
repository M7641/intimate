"""Discover a categorical taxonomy from free-text predictions.

The `generic` schema generates a free-text `product_type`, which has
unbounded cardinality (synonyms, spellings, near-duplicates) — noisy training
targets. This clusters those values into a compact set of canonical labels: the
data-driven `choices` you then freeze into a CATEGORICAL schema (discover, then
constrain).

The cosine merge threshold is the concentration dial: lower merges more
aggressively (fewer, broader clusters → lower cardinality, higher support per
class); higher keeps finer distinctions. Sweep it and watch `concentration`.
"""

import math

import polars as pl
import torch
from transformers import AutoModel, AutoTokenizer


def embed(texts, model_id="sentence-transformers/all-MiniLM-L6-v2"):
    """Mean-pooled, L2-normalised sentence embeddings (one row per text).

    Rows are unit vectors, so a dot product between any two is their cosine
    similarity. Loads a small encoder via the transformers stack already in the
    project — no extra dependency.
    """
    tokenizer = AutoTokenizer.from_pretrained(model_id)
    model = AutoModel.from_pretrained(model_id).eval()
    enc = tokenizer(list(texts), padding=True, truncation=True, return_tensors="pt")
    with torch.no_grad():
        hidden = model(**enc).last_hidden_state  # (n, seq, dim)
    mask = enc["attention_mask"].unsqueeze(-1).type_as(hidden)
    pooled = (hidden * mask).sum(1) / mask.sum(1).clamp(min=1e-9)
    return pooled / pooled.norm(dim=1, keepdim=True).clamp(min=1e-9)


def cluster(embeddings, threshold=0.65):
    """Greedy single-pass clustering by cosine similarity to a running centroid.

    Expects ``embeddings`` ordered most-frequent-first, so each new cluster is
    seeded by (and canonicalised to) its most common surface form. Returns a
    list of clusters, each a list of row indices into ``embeddings``.
    """
    centroids: list[torch.Tensor] = []
    groups: list[list[int]] = []
    for i in range(embeddings.shape[0]):
        vector = embeddings[i]
        best, best_sim = -1, -1.0
        for c, centroid in enumerate(centroids):
            sim = float(torch.dot(vector, centroid))
            if sim > best_sim:
                best, best_sim = c, sim
        if best_sim >= threshold:
            groups[best].append(i)
            centroid = embeddings[groups[best]].mean(0)
            centroids[best] = centroid / centroid.norm().clamp(min=1e-9)
        else:
            groups.append([i])
            centroids.append(vector.clone())
    return groups


def discover_taxonomy(
    df,
    levels=("product_type",),
    threshold=0.65,
    model_id="sentence-transformers/all-MiniLM-L6-v2",
):
    """Cluster each level's free-text values into canonical labels.

    Returns one row per cluster: ``level``, ``canonical`` (the label),
    ``support`` (rows that map to it), ``members`` (the raw values it absorbed).
    Embeds the whole vocabulary once and reuses it across levels.
    """
    schema = {
        "level": pl.Utf8,
        "canonical": pl.Utf8,
        "support": pl.Int64,
        "members": pl.List(pl.Utf8),
    }
    per_level: dict[str, tuple[list[str], list[int]]] = {}
    vocabulary: list[str] = []
    for level in levels:
        if level not in df.columns:
            continue
        counts = df.get_column(level).drop_nulls().cast(pl.Utf8).value_counts(sort=True)
        values = counts.get_column(level).to_list()
        if not values:
            continue
        per_level[level] = (values, counts.get_column("count").to_list())
        vocabulary.extend(values)

    if not vocabulary:
        return pl.DataFrame(schema=schema)

    unique = list(dict.fromkeys(vocabulary))  # dedupe, preserve order
    index = {value: i for i, value in enumerate(unique)}
    embeddings = embed(unique, model_id)

    rows = []
    for level, (values, counts) in per_level.items():
        local = embeddings[[index[v] for v in values]]
        for group in cluster(local, threshold):
            rows.append(
                {
                    "level": level,
                    "canonical": values[group[0]],
                    "support": sum(counts[i] for i in group),
                    "members": [values[i] for i in group],
                }
            )
    return pl.DataFrame(rows, schema=schema)


def concentration(taxonomy):
    """Per-level distribution health: how concentrated are the labels?

    ``clusters`` is the post-merge cardinality, ``raw_values`` the pre-merge
    count (their ratio is the compression). ``norm_entropy`` in [0, 1] is the
    Shannon entropy of cluster supports over log(clusters): ~0 means one class
    dominates (little signal), ~1 means uniform; a useful band sits in between.
    ``top_share`` is the largest class's fraction.
    """
    rows = []
    for (level,), group in taxonomy.group_by("level"):
        supports = group.get_column("support").to_list()
        total = sum(supports) or 1
        probs = [s / total for s in supports]
        entropy = -sum(p * math.log(p) for p in probs if p > 0)
        k = len(supports)
        rows.append(
            {
                "level": level,
                "raw_values": int(group.get_column("members").list.len().sum()),
                "clusters": k,
                "norm_entropy": round(entropy / math.log(k), 3) if k > 1 else 0.0,
                "top_share": round(max(probs), 3) if probs else 0.0,
            }
        )
    return pl.DataFrame(rows)


def to_choices(taxonomy):
    """Per level, the canonical labels (support-descending) — the `choices` to
    freeze into a CATEGORICAL schema."""
    out = {}
    for (level,), group in taxonomy.group_by("level"):
        out[level] = (
            group.sort("support", descending=True).get_column("canonical").to_list()
        )
    return out
