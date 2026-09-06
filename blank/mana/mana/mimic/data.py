"""Dataset, batching, and collation for variable-length sets."""

from __future__ import annotations

from dataclasses import dataclass

import torch
from torch import Tensor
from torch.utils.data import DataLoader, Dataset


@dataclass
class SetBatch:
    """A padded batch of sets with mask.

    x:    (B, N_max, D) — padded set elements
    mask: (B, N_max)    — True = valid, False = padded
    ids:  optional entity IDs aligned with batch rows
    """

    x: Tensor
    mask: Tensor
    ids: list[int | str] | None = None


class SetDataset(Dataset[Tensor]):
    """Wraps a list of variable-length set tensors.

    Each tensor has shape (N_i, D) where N_i may vary across samples.
    Optionally carries entity IDs aligned with the sets.
    """

    def __init__(self, sets: list[Tensor], ids: list[int | str] | None = None) -> None:
        self.sets = sets
        self.ids = ids
        if ids is not None and len(ids) != len(sets):
            raise ValueError(f"ids length ({len(ids)}) != sets length ({len(sets)})")

    def __len__(self) -> int:
        return len(self.sets)

    def __getitem__(self, idx: int) -> Tensor | tuple[Tensor, int | str]:  # type: ignore[override]
        if self.ids is not None:
            return self.sets[idx], self.ids[idx]
        return self.sets[idx]


def collate_sets(batch: list) -> SetBatch:
    """Pad variable-length sets to the max length in the batch.

    Accepts either bare tensors (N_i, D) or (tensor, id) tuples
    (produced by SetDataset with ids).
    Returns a SetBatch with padded tensors, boolean mask, and optional ids.
    """
    ids: list[int | str] | None = None
    if isinstance(batch[0], tuple):
        tensors = [item[0] for item in batch]
        ids = [item[1] for item in batch]
    else:
        tensors = batch

    max_len = max(t.shape[0] for t in tensors)
    dim = tensors[0].shape[1]
    B = len(tensors)

    x = torch.zeros(B, max_len, dim)
    mask = torch.zeros(B, max_len, dtype=torch.bool)

    for i, t in enumerate(tensors):
        n = t.shape[0]
        x[i, :n] = t
        mask[i, :n] = True

    return SetBatch(x=x, mask=mask, ids=ids)


def set_dataloader(
    dataset: SetDataset,
    # TUNING: batch_size is critical for contrastive learning — each sample
    # in the batch acts as a negative for every other sample. Larger batches
    # (64-256) give more negatives and a better loss signal. Small batches
    # (<32) may need strategy="debiased" in NTXentLoss to compensate.
    # Upper limit is GPU memory; diminishing returns above 512.
    batch_size: int = 32,
    shuffle: bool = True,
    num_workers: int = 0,
    **kwargs: object,
) -> DataLoader[SetBatch]:
    """Convenience wrapper for DataLoader with set collation."""
    return DataLoader(
        dataset,
        batch_size=batch_size,
        shuffle=shuffle,
        num_workers=num_workers,
        collate_fn=collate_sets,
        **kwargs,  # type: ignore[override]
    )
