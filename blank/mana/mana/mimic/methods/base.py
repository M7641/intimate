"""BaseMethod: shared encoder ownership and inference for all SSL methods."""

from __future__ import annotations

from pathlib import Path

import torch
import torch.nn as nn
from torch import Tensor
from torch.utils.data import DataLoader

from ..data import SetBatch
from ..types import SetEncoder


class BaseMethod(nn.Module):
    """Base class for self-supervised methods (axis B).

    A method owns the shared `encoder` and turns a batch into a training loss
    via `training_step`. Inference (`encode`, `encode_all`) reads only the
    encoder, so it is identical across every method and lives here once.

    `forward` delegates to `training_step`, so a method stays callable as
    `method(x, mask)` — keeping it a drop-in for the training loop and any
    code that treats it as a plain nn.Module.

    Subclasses must set `self.encoder` in `__init__` and implement
    `training_step`.
    """

    encoder: SetEncoder

    def training_step(
        self, x: Tensor, mask: Tensor | None = None, labels: Tensor | None = None
    ) -> Tensor:
        raise NotImplementedError

    def forward(
        self, x: Tensor, mask: Tensor | None = None, labels: Tensor | None = None
    ) -> Tensor:
        return self.training_step(x, mask, labels)

    @torch.no_grad()
    def encode(self, x: Tensor, mask: Tensor | None = None) -> Tensor:
        """Encode sets without augmentation. Returns embeddings (B, D)."""
        was_training = self.training
        self.set_eval_mode()
        h = self.encoder(x, mask)
        if was_training:
            self.train()
        return h

    @torch.no_grad()
    def encode_all(
        self, dataloader: DataLoader[SetBatch], verbose: bool = False
    ) -> Tensor:
        """Encode all batches from a dataloader, returning concatenated embeddings."""
        was_training = self.training
        self.set_eval_mode()
        parts: list[Tensor] = []
        for i, batch in enumerate(dataloader):
            if verbose:
                print(f"  encoding batch {i + 1}/{len(dataloader)}", flush=True)
            parts.append(self.encoder(batch.x, batch.mask))
        if was_training:
            self.train()
        return torch.cat(parts, dim=0)

    def set_eval_mode(self) -> None:
        """Switch to evaluation mode."""
        nn.Module.eval(self)

    def save(self, path: str | Path) -> None:
        """Save model state dict to disk."""
        torch.save(self.state_dict(), path)

    def load(
        self, path: str | Path, map_location: str | torch.device | None = None
    ) -> None:
        """Load state dict from disk into this model."""
        self.load_state_dict(
            torch.load(path, map_location=map_location, weights_only=True)
        )
