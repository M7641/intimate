"""A1 — embeddings as direct features (README).

Two phases. The *embedding pass* encodes each distinct description into a
unit vector with a modern retrieval encoder (Qwen3-Embedding by default),
loaded once and run in chunks so a whole catalogue sample is one cheap sweep.
The *compression phase* projects those vectors down to a small fixed width
(default 16) with PCA, so the stored feature neither swamps a tree model nor
balloons storage.

The fitted projection is data, not code: it is persisted next to the
embeddings and reused on every later run, so new products land in the SAME
space as the stored vectors. Refitting would silently shift every feature.

PCA over UMAP, deliberately: PCA is a stored linear map (a mean + a matrix) —
JSON-serialisable, deterministic, dependency-free (torch SVD), and exact on
out-of-sample rows. UMAP preserves local structure better but must pickle its
fitted model and its transform of new points is approximate — the wrong trade
for an incremental feature pipeline. If UMAP earns its keep later it can hide
behind the same ``projection`` interface.
"""

from __future__ import annotations

from collections.abc import Callable

import polars as pl
import torch
from transformers import AutoModel, AutoTokenizer

from extract.dedup import description_key, normalise

# A modern retrieval encoder, per the A1 "upgrade" note: Qwen3-Embedding
# (mid-2025) — top-tier MTEB retrieval, 1024-dim, 32k context (long product
# copy fits untruncated, where MiniLM cuts at 512), and the same model family
# the extractor already runs (Qwen2.5 checkpoints). MiniLM stays the encoder
# for discover/condense: their persisted centroids were built in its space.
DEFAULT_ENCODER = "Qwen/Qwen3-Embedding-0.6B"
DEFAULT_DIM = 16

# Encoders are trained to read the embedding off different positions: the
# Qwen3-Embedding family off the LAST attended token (causal arch), the
# BGE/GTE/Arctic family off [CLS], the older sentence-transformers family via
# masked mean pooling. Pooling with the wrong strategy silently degrades
# quality, so resolve it from the checkpoint name (overridable via
# Encoder(pooling=...)).
LAST_FAMILIES = ("qwen3-embedding",)
CLS_FAMILIES = ("bge", "gte", "arctic", "modernbert")


def default_pooling(model_id: str) -> str:
    """The pooling strategy a checkpoint was trained for, by name."""
    name = model_id.lower()
    if any(family in name for family in LAST_FAMILIES):
        return "last"
    if any(family in name for family in CLS_FAMILIES):
        return "cls"
    return "mean"


def pool(hidden: torch.Tensor, mask: torch.Tensor, pooling: str) -> torch.Tensor:
    """Reduce (n, seq, dim) hidden states to one L2-normalised row per text."""
    if pooling == "last":
        # The last attended position per row. With left padding (the Qwen
        # tokenizer default) that is simply the final column; with right
        # padding, gather each row's last unmasked index — the canonical
        # last_token_pool from the Qwen3-Embedding model card.
        if bool(mask[:, -1].all()):
            pooled = hidden[:, -1]
        else:
            indices = mask.sum(dim=1) - 1
            pooled = hidden[torch.arange(hidden.shape[0]), indices]
    elif pooling == "cls":
        pooled = hidden[:, 0]
    elif pooling == "mean":
        weights = mask.unsqueeze(-1).type_as(hidden)
        pooled = (hidden * weights).sum(1) / weights.sum(1).clamp(min=1e-9)
    else:
        msg = f"unknown pooling {pooling!r}; use 'last', 'cls' or 'mean'"
        raise ValueError(msg)
    return pooled / pooled.norm(dim=1, keepdim=True).clamp(min=1e-9)


class Encoder:
    """The sentence encoder, loaded once, pooled to match its checkpoint."""

    def __init__(
        self, model_id: str = DEFAULT_ENCODER, *, pooling: str | None = None
    ) -> None:
        self.model_id = model_id
        self.pooling = pooling or default_pooling(model_id)
        self.tokenizer = AutoTokenizer.from_pretrained(model_id)
        self.model = AutoModel.from_pretrained(model_id).eval()

    @torch.no_grad()
    def encode(self, texts: list[str]) -> torch.Tensor:
        """L2-normalised float32 embeddings — one unit row per text.

        float32 at the boundary regardless of the model's compute dtype:
        Qwen3 runs in bfloat16, which CPU SVD (the PCA fit) cannot take.
        """
        enc = self.tokenizer(
            list(texts), padding=True, truncation=True, return_tensors="pt"
        )
        hidden = self.model(**enc).last_hidden_state
        return pool(hidden, enc["attention_mask"], self.pooling).float()

    @torch.no_grad()
    def encode_batched(
        self,
        texts: list[str],
        batch_size: int = 64,
        on_progress: Callable[[int, int], None] | None = None,
    ) -> torch.Tensor:
        """Encode in chunks: bounded padding/memory, one model load for all."""
        chunks = []
        for start in range(0, len(texts), batch_size):
            chunks.append(self.encode(texts[start : start + batch_size]))
            if on_progress:
                on_progress(min(start + batch_size, len(texts)), len(texts))
        return torch.cat(chunks)


def fit_projection(vectors: torch.Tensor, dim: int = DEFAULT_DIM) -> dict:
    """Fit a PCA projection: the mean + the top ``dim`` principal directions.

    Plain SVD on the centred matrix — no new dependency. The returned dict is
    JSON-ready so it can be persisted; see the module docstring for why the
    projection must outlive the run.
    """
    if vectors.shape[0] < dim:
        msg = (
            f"need at least {dim} distinct descriptions to fit a {dim}-dim "
            f"projection, got {vectors.shape[0]} — sample more on the first run"
        )
        raise ValueError(msg)
    mean = vectors.mean(dim=0)
    _, _, vh = torch.linalg.svd(vectors - mean, full_matrices=False)
    return {"dim": dim, "mean": mean.tolist(), "components": vh[:dim].tolist()}


def apply_projection(vectors: torch.Tensor, projection: dict) -> torch.Tensor:
    """Project embeddings into a stored PCA space: centre, then rotate down."""
    mean = torch.tensor(projection["mean"], dtype=vectors.dtype)
    components = torch.tensor(projection["components"], dtype=vectors.dtype)
    return (vectors - mean) @ components.T


def embed_frame(
    df: pl.DataFrame,
    encoder: Encoder,
    *,
    description_col: str = "description",
    dim: int = DEFAULT_DIM,
    projection: dict | None = None,
    batch_size: int = 64,
    on_progress: Callable[[int, int], None] | None = None,
) -> tuple[pl.DataFrame, dict]:
    """Add an ``embedding`` column (list of ``dim`` floats) to ``df``.

    The embedding pass runs once per DISTINCT normalised description (Step-0
    dedup — free here, since encoding is deterministic) and broadcasts to the
    duplicate rows. ``projection=None`` fits a fresh PCA on this frame's
    distinct vectors; pass a stored projection on every later run. Returns
    ``(df_with_embedding, projection_used)`` — persist the projection when it
    was freshly fitted.
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
    if projection is None:
        projection = fit_projection(vectors, dim)
    compressed = apply_projection(vectors, projection)

    by_key = {key: compressed[i].tolist() for i, key in enumerate(first)}
    column = pl.Series("embedding", [by_key[key] for key in keys])
    return df.with_columns(column), projection
