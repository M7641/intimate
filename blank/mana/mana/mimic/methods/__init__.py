"""Self-supervised methods (axis B) — objectives that shape the embedding space.

Each method owns a shared encoder (axis A) and turns a batch into a training
loss. The trainer is method-agnostic, so any encoder pairs with any method.
"""

from .base import BaseMethod
from .contrastive import ContrastiveMethod
from .mae import MaskedSetAutoencoder

__all__ = [
    "BaseMethod",
    "ContrastiveMethod",
    "MaskedSetAutoencoder",
]
