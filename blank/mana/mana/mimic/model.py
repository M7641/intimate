"""Backward-compatible alias for the contrastive method.

`MimicModel` was the single, hard-coded contrastive model. It is now one point
on axis B — `ContrastiveMethod` — sitting alongside other objectives (masked
autoencoding, redundancy reduction, ...). The name and constructor are kept so
existing code (`mimic.MimicModel(encoder=..., projector=..., ...)`) is unchanged.

Prefer `ContrastiveMethod` in new code.
"""

from __future__ import annotations

from .methods.contrastive import ContrastiveMethod, _masked_mean

MimicModel = ContrastiveMethod

__all__ = ["MimicModel", "_masked_mean"]
