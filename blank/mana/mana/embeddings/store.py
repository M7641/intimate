"""EmbeddingStore: serializable container mapping entity IDs to embedding vectors.

Embedding engines (mimic, future time-series, etc.) produce these.
Adapters (recommender, forecast, etc.) consume these.
Serve (ANN) indexes these.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import numpy as np
import torch
from torch import Tensor
from torch.utils.data import DataLoader, Dataset


@dataclass
class EmbeddingStore:
    """Maps entity IDs to embedding vectors.

    Args:
        ids: Entity identifiers (ints or strings), one per row.
        embeddings: (N, D) tensor of embedding vectors.
        metadata: Arbitrary info about how the embeddings were produced.
    """

    ids: list[int | str]
    embeddings: Tensor
    metadata: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if len(self.ids) != self.embeddings.shape[0]:
            raise ValueError(
                f"ids length ({len(self.ids)}) != embeddings rows ({self.embeddings.shape[0]})"
            )

    def __len__(self) -> int:
        return len(self.ids)

    def __getitem__(self, key: int | str | list[int | str]) -> Tensor:
        """Look up embeddings by entity ID(s).

        Returns (D,) for a single ID, (N, D) for a list of IDs.
        """
        if isinstance(key, list):
            indices = [self.ids.index(k) for k in key]
            return self.embeddings[indices]
        return self.embeddings[self.ids.index(key)]

    def split(
        self, test_ratio: float = 0.2, seed: int | None = None
    ) -> tuple[EmbeddingStore, EmbeddingStore]:
        """Split into train/test EmbeddingStores by random permutation."""
        n = len(self)
        n_test = int(n * test_ratio)
        gen = torch.Generator()
        if seed is not None:
            gen.manual_seed(seed)
        perm = torch.randperm(n, generator=gen)
        train_idx = perm[: n - n_test]
        test_idx = perm[n - n_test :]
        return (
            EmbeddingStore(
                ids=[self.ids[i] for i in train_idx.tolist()],
                embeddings=self.embeddings[train_idx],
                metadata=self.metadata,
            ),
            EmbeddingStore(
                ids=[self.ids[i] for i in test_idx.tolist()],
                embeddings=self.embeddings[test_idx],
                metadata=self.metadata,
            ),
        )

    @property
    def dim(self) -> int:
        """Embedding dimensionality."""
        return self.embeddings.shape[1]

    def to_numpy(self) -> np.ndarray:
        """Return embeddings as a NumPy array."""
        return self.embeddings.detach().cpu().numpy()

    def to_dict(self) -> dict[int | str, Tensor]:
        """Return a mapping from each ID to its embedding vector."""
        return {id_: self.embeddings[i] for i, id_ in enumerate(self.ids)}

    def save(self, path: str | Path) -> None:
        """Serialize to disk via torch.save."""
        torch.save(
            {
                "ids": self.ids,
                "embeddings": self.embeddings,
                "metadata": self.metadata,
            },
            path,
        )

    @classmethod
    def load(cls, path: str | Path) -> EmbeddingStore:
        """Deserialize from a file saved with .save()."""
        data = torch.load(path, weights_only=False)
        return cls(
            ids=data["ids"],
            embeddings=data["embeddings"],
            metadata=data.get("metadata", {}),
        )

    @classmethod
    def from_mimic(
        cls,
        model: torch.nn.Module,
        dataloader_or_dataset: DataLoader | Dataset,
        ids: list[int | str] | None = None,
        metadata: dict[str, Any] | None = None,
        batch_size: int | None = None,
    ) -> EmbeddingStore:
        """Create an EmbeddingStore by running model.encode_all().

        Accepts either a DataLoader or a SetDataset directly. When a SetDataset
        with ids is passed, ids are extracted automatically.

        Args:
            model: A trained MimicModel (or anything with encode_all(dataloader)).
            dataloader_or_dataset: DataLoader or SetDataset.
            ids: Entity IDs. Optional when the dataset carries its own ids.
            metadata: Optional extra metadata to attach.
            batch_size: Batch size when wrapping a dataset (defaults to len(dataset)).
        """
        from mana.mimic.data import SetDataset, set_dataloader

        if isinstance(dataloader_or_dataset, SetDataset):
            dataset = dataloader_or_dataset
            effective_ids = ids if ids is not None else dataset.ids
            if effective_ids is None:
                raise ValueError(
                    "ids must be provided via SetDataset or ids= parameter"
                )
            bs = batch_size if batch_size is not None else len(dataset)
            dl = set_dataloader(dataset, batch_size=bs, shuffle=False)
        else:
            dl = dataloader_or_dataset
            effective_ids = ids
            if effective_ids is None:
                raise ValueError("ids required when passing a DataLoader")

        embeddings = model.encode_all(dl)
        meta = {"source": "mimic"}
        if metadata:
            meta.update(metadata)
        return cls(ids=effective_ids, embeddings=embeddings, metadata=meta)

    @classmethod
    def from_pretrained(
        cls,
        embeddings: Tensor | np.ndarray,
        ids: list[int | str],
        metadata: dict[str, Any] | None = None,
    ) -> EmbeddingStore:
        """Wrap externally-produced embeddings.

        Args:
            embeddings: (N, D) tensor or numpy array.
            ids: Entity IDs.
            metadata: Optional extra metadata.
        """
        if isinstance(embeddings, np.ndarray):
            embeddings = torch.from_numpy(embeddings)
        meta = {"source": "pretrained"}
        if metadata:
            meta.update(metadata)
        return cls(ids=ids, embeddings=embeddings, metadata=meta)
