"""Protocol definitions for swappable components."""

from __future__ import annotations

from typing import Protocol

from torch import Tensor


class Augmentation(Protocol):
    """Augments a set tensor, potentially modifying its mask.

    Returns (augmented_x, updated_mask_or_None).
    """

    def __call__(
        self, x: Tensor, mask: Tensor | None = None
    ) -> tuple[Tensor, Tensor | None]: ...


class ElementEncoder(Protocol):
    """Encodes individual set elements: (B, N, D_in) → (B, N, D_out)."""

    def __call__(self, x: Tensor) -> Tensor: ...


class SetEncoder(Protocol):
    """Full set encoder: (B, N, D_in) → (B, D_out)."""

    def __call__(self, x: Tensor, mask: Tensor | None = None) -> Tensor: ...


class Projector(Protocol):
    """Projects set embeddings to contrastive space: (B, D_in) → (B, D_out)."""

    def __call__(self, x: Tensor) -> Tensor: ...


class ContrastiveLoss(Protocol):
    """Computes contrastive loss between two batches of projections.

    Accepts optional labels for supervised contrastive methods.
    """

    def __call__(
        self, z1: Tensor, z2: Tensor, labels: Tensor | None = None
    ) -> Tensor: ...


class Aggregator(Protocol):
    """Aggregates encoded elements into a fixed-size set representation.

    (B, N, D) → (B, D). Mask: True = valid, False = padded.
    """

    def __call__(self, x: Tensor, mask: Tensor | None = None) -> Tensor: ...


class Method(Protocol):
    """A self-supervised objective — the axis-B abstraction.

    A method owns the shared `encoder` (axis A) and turns one batch into a
    scalar training loss via `training_step`. It encapsulates everything that
    distinguishes one objective from another: the number of views, the
    projector/predictor, stop-gradients, a teacher network, a decoder.

    The trainer is method-agnostic — it only calls `training_step` and reads
    `encoder`. Any encoder paired with any method is a valid combination.
    """

    encoder: SetEncoder

    def training_step(
        self, x: Tensor, mask: Tensor | None = None, labels: Tensor | None = None
    ) -> Tensor: ...
