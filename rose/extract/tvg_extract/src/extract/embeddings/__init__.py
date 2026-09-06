"""A1 embeddings + PCA compression — see :mod:`extract.embeddings.embeddings`."""

from extract.embeddings.embeddings import (
    DEFAULT_DIM,
    DEFAULT_ENCODER,
    Encoder,
    apply_projection,
    default_pooling,
    embed_frame,
    fit_projection,
    pool,
)

__all__ = [
    "DEFAULT_DIM",
    "DEFAULT_ENCODER",
    "Encoder",
    "apply_projection",
    "default_pooling",
    "embed_frame",
    "fit_projection",
    "pool",
]
