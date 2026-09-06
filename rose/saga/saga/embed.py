"""Deterministic hashing embedding — the *stand-in* for the embedding model.

In production this module is replaced by a call to a real encoder (OpenAI
`text-embedding-3`, BGE, nomic-embed, Voyage...). What matters for the pilot is
that the *shape* is identical: `embed(text) -> normalised vector`, and that the
**cosine similarity** between two close texts is > that of two distant ones. A
token-hashing embedding (the "hashing trick" / feature hashing) is enough to
demonstrate that offline, without downloading 400 MB of weights.

Properties deliberately kept vs a real model:
  * deterministic (same text -> same vector, no hidden seed — we use `hashlib`,
    not `hash()` which is salted per process);
  * bounded dimensionality (`DIM`);
  * L2-normalised vector, so `cosine(a, b) == dot(a, b)`.

What a real model adds (and which we do NOT simulate): semantics. Here "car" and
"automobile" are orthogonal; a real encoder would pull them together. That is
precisely the boundary of the seam.
"""

from __future__ import annotations

import hashlib
import math
import re
from array import array

DIM = 256
_TOKEN = re.compile(r"[a-z0-9]+")


def _tokens(text: str) -> list[str]:
    """Unigrams + bigrams, lowercased. Bigrams give a little ordering signal
    ("strong typing" != "typing strong")."""
    words = _TOKEN.findall(text.lower())
    bigrams = [f"{a}_{b}" for a, b in zip(words, words[1:])]
    return words + bigrams


def _bucket(token: str) -> tuple[int, float]:
    """Hash a token to (dimension index, sign). Two bits of the hash: one picks
    the dimension, the other the sign — the sign decorrelates collisions instead
    of stacking them."""
    h = hashlib.blake2b(token.encode("utf-8"), digest_size=8).digest()
    idx = int.from_bytes(h[:4], "big") % DIM
    sign = 1.0 if h[4] & 1 else -1.0
    return idx, sign


def embed(text: str) -> array:
    """Text -> L2-normalised `float32` vector of dimension `DIM`."""
    vec = array("f", [0.0] * DIM)
    for tok in _tokens(text):
        idx, sign = _bucket(tok)
        vec[idx] += sign
    norm = math.sqrt(sum(x * x for x in vec))
    if norm > 0.0:
        for i in range(DIM):
            vec[i] /= norm
    return vec


def cosine(a: array, b: array) -> float:
    """Cosine similarity. Vectors already normalised -> plain dot product."""
    return sum(x * y for x, y in zip(a, b))


def to_blob(vec: array) -> bytes:
    """Serialise for the SQLite BLOB column."""
    return vec.tobytes()


def from_blob(blob: bytes) -> array:
    """Deserialise from the SQLite BLOB column."""
    vec = array("f")
    vec.frombytes(blob)
    return vec
