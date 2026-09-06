"""Sequential augmentation pipeline."""

from __future__ import annotations

import torch
import torch.nn as nn
from torch import Tensor

from .types import Augmentation


class Compose(nn.Module):
    """Applies a sequence of augmentations, threading mask through."""

    def __init__(self, augmentations: list[Augmentation]) -> None:
        super().__init__()
        self.augmentations = nn.ModuleList(augmentations)

    def forward(
        self, x: Tensor, mask: Tensor | None = None
    ) -> tuple[Tensor, Tensor | None]:
        for aug in self.augmentations:
            x, mask = aug(x, mask)
        return x, mask


class CurriculumAugmentation(nn.Module):
    """Wraps an augmentation and linearly ramps its strength.

    Interpolates augmentation parameters between start and end values
    based on a progress fraction [0, 1]. Call set_progress() each epoch.

    Works with augmentations that have numeric attributes controlling
    their strength: 'std' (FeatureNoise), 'p' (FeatureMask),
    'keep_fraction' (SubsetSample).
    """

    def __init__(
        self,
        augmentation: Augmentation,
        start_strength: float = 0.1,
        end_strength: float = 1.0,
    ) -> None:
        super().__init__()
        self.augmentation = augmentation
        self.start_strength = start_strength
        self.end_strength = end_strength
        self._progress = 0.0

        # Snapshot original parameter values
        self._orig_values: dict[str, float] = {}
        for attr in ("std", "p", "keep_fraction"):
            if hasattr(self.augmentation, attr):
                self._orig_values[attr] = getattr(self.augmentation, attr)

    def set_progress(self, fraction: float) -> None:
        """Set training progress [0.0 = start, 1.0 = end]."""
        self._progress = max(0.0, min(1.0, fraction))
        strength = (
            self.start_strength
            + (self.end_strength - self.start_strength) * self._progress
        )

        for attr, orig in self._orig_values.items():
            scaled = orig * strength
            if attr == "keep_fraction":
                # keep_fraction should decrease with strength (more dropout)
                # At strength=0, keep everything; at strength=1, use original keep_fraction
                scaled = 1.0 - (1.0 - orig) * strength
            setattr(self.augmentation, attr, scaled)

    def forward(
        self, x: Tensor, mask: Tensor | None = None
    ) -> tuple[Tensor, Tensor | None]:
        return self.augmentation(x, mask)


class FeatureMask(nn.Module):
    """Randomly zeros out features (columns) during training.

    Each feature dimension is independently zeroed with probability p.
    Applied per-sample, same mask across all set elements within a sample.

    Input:  (B, N, D)
    Output: (B, N, D) with features zeroed, set mask unchanged
    """

    def __init__(
        self,
        # TUNING: p controls how much feature information is dropped per view.
        # Too low (0.01) = views are nearly identical, trivial task, no learning.
        # Too high (0.5+) = too much information destroyed, model can't find
        # the invariance. Start at 0.1-0.2. Increase if loss converges too fast
        # (model finds a shortcut), decrease if loss doesn't converge at all.
        p: float = 0.15,
    ) -> None:
        super().__init__()
        self.p = p

    def forward(
        self, x: Tensor, mask: Tensor | None = None
    ) -> tuple[Tensor, Tensor | None]:
        if self.training:
            # Feature mask: (B, 1, D) — broadcast across set elements
            feat_mask = torch.bernoulli(
                torch.full((x.shape[0], 1, x.shape[-1]), 1.0 - self.p, device=x.device)
            )
            x = x * feat_mask
        return x, mask


class FeatureNoise(nn.Module):
    """Adds Gaussian noise to features during training only.

    Input:  (B, N, D)
    Output: (B, N, D) with noise added, mask unchanged
    """

    def __init__(
        self,
        # TUNING: std controls noise magnitude relative to your feature scale.
        # If features are normalised to ~unit variance, 0.05-0.2 is a good range.
        # If features have large magnitude, scale std accordingly. Noise that is
        # too weak doesn't create a meaningful learning signal; too strong and the
        # model learns to ignore features entirely.
        std: float = 0.1,
    ) -> None:
        super().__init__()
        self.std = std

    def forward(
        self, x: Tensor, mask: Tensor | None = None
    ) -> tuple[Tensor, Tensor | None]:
        if self.training:
            x = x + torch.randn_like(x) * self.std
        return x, mask


class SubsetSample(nn.Module):
    """Randomly drops set elements by modifying the mask during training.

    Each valid element is independently kept with probability keep_fraction.
    At least one element per sample is always kept.
    """

    def __init__(
        self,
        # TUNING: keep_fraction controls how many set elements survive per view.
        # 0.7-0.9 works well for most cases. Lower values (0.5) force the model
        # to learn from partial observations — useful when sets are large and
        # redundant. Too aggressive on small sets (3-5 elements) risks losing
        # all meaningful structure. Combine with CurriculumAugmentation to
        # ramp from gentle (0.95) to aggressive (0.6) during training.
        keep_fraction: float = 0.8,
    ) -> None:
        super().__init__()
        self.keep_fraction = keep_fraction

    def forward(
        self, x: Tensor, mask: Tensor | None = None
    ) -> tuple[Tensor, Tensor | None]:
        if not self.training:
            return x, mask

        B, N, _ = x.shape

        if mask is None:
            mask = torch.ones(B, N, dtype=torch.bool, device=x.device)

        # Sample which elements to keep (among currently valid ones)
        keep_prob = torch.full((B, N), self.keep_fraction, device=x.device)
        keep = torch.bernoulli(keep_prob).bool()

        new_mask = mask & keep

        # Ensure at least one element per sample
        empty = ~new_mask.any(dim=1)
        if empty.any():
            # For empty samples, restore a random valid element
            for i in empty.nonzero(as_tuple=False).squeeze(-1):
                valid_indices = mask[i].nonzero(as_tuple=False).squeeze(-1)
                chosen = valid_indices[torch.randint(len(valid_indices), (1,))]
                new_mask[i, chosen] = True

        return x, new_mask
