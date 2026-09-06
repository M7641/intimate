from collections import defaultdict

import numpy as np
import polars as pl
import torch
from torch.utils.data import Sampler


class UniqueQueryBatchSampler(Sampler):
    """
    Custom batch sampler that ensures each batch contains only unique query_ids.

    For queries with multiple interactions, they are distributed across different batches
    using a round-robin strategy, ensuring all rows are used in each epoch.

    Example:
        If query_id=100 has 3 interactions at indices [0, 5, 12] and batch_size=32:
        - Epoch 1: index 0 might be in batch 1, index 5 in batch 3, index 12 in batch 7
        - Epoch 2: These get reshuffled but still distributed across different batches
    """

    def __init__(
        self,
        dataset: torch.utils.data.Dataset,
        batch_size: int,
        shuffle: bool = True,
        drop_last: bool = False,
        query_to_indices: dict[int, list[int]] | None = None,
    ) -> None:
        self.dataset = dataset
        self.batch_size = batch_size
        self.shuffle = shuffle
        self.drop_last = drop_last

        if query_to_indices is not None:
            self.query_to_indices = {k: list(v) for k, v in query_to_indices.items()}
        else:
            # Group indices by query_id (O(n) iteration through dataset)
            self.query_to_indices = defaultdict(list)
            dataset_len = len(dataset)  # type: ignore[arg-type]
            for idx in range(dataset_len):
                item = dataset[idx]
                query_id = (
                    item["query_id"].item()
                    if isinstance(item["query_id"], torch.Tensor)
                    else item["query_id"]
                )
                self.query_to_indices[query_id].append(idx)

        # Store query_ids as a list for consistent ordering
        self.query_ids = list(self.query_to_indices.keys())

    def __iter__(self):
        """
        Generate batches with unique query_ids using a round-robin approach.
        """
        # Shuffle interactions within each query
        query_interaction_lists = {}
        for query_id in self.query_ids:
            indices = self.query_to_indices[query_id].copy()
            if self.shuffle:
                np.random.shuffle(indices)
            query_interaction_lists[query_id] = indices

        # Distribute into rounds using round-robin
        # Find max number of interactions any query has
        max_interactions = max(
            len(indices) for indices in query_interaction_lists.values()
        )

        all_indices = []
        for round_idx in range(max_interactions):
            # Get one interaction per query for this round (if available)
            round_indices = []
            for query_id in self.query_ids:
                if round_idx < len(query_interaction_lists[query_id]):
                    round_indices.append(query_interaction_lists[query_id][round_idx])

            # Shuffle within this round
            if self.shuffle:
                np.random.shuffle(round_indices)

            all_indices.extend(round_indices)

        # Create batches from the organized indices
        batches = []
        for i in range(0, len(all_indices), self.batch_size):
            batch = all_indices[i : i + self.batch_size]
            if len(batch) == self.batch_size or not self.drop_last:
                batches.append(batch)

        return iter(batches)

    def __len__(self) -> int:
        """Return the number of batches."""
        total_samples = len(self.dataset)  # type: ignore[arg-type]
        if self.drop_last:
            return total_samples // self.batch_size
        return (total_samples + self.batch_size - 1) // self.batch_size


class InteractionDataset(torch.utils.data.Dataset):
    def __init__(
        self,
        dataset: pl.DataFrame,
        embedding_dim: int,
        target_variable: str | None = None,
    ) -> None:
        self.dataset = dataset
        self.embedding_dim = embedding_dim
        self.target_variable = target_variable

    def __len__(self) -> int:
        return self.dataset.height

    def __getitem__(self, index: int) -> dict[str, torch.Tensor]:
        row_val = self.dataset.row(index=index, named=True)

        return_item = {
            "query_id": torch.tensor(row_val["query_id"], dtype=torch.long),
            "candidate_id": torch.tensor(row_val["candidate_id"], dtype=torch.long),
            "query_embedding": torch.tensor(
                row_val["query_embedding"], dtype=torch.float
            ),
            "candidate_embedding": torch.tensor(
                row_val["candidate_embedding"], dtype=torch.float
            ),
        }

        if self.target_variable:
            return_item[self.target_variable] = torch.tensor(
                row_val[self.target_variable],
                dtype=torch.float,
            )

        return return_item


class InteractionDataLoader:
    def __init__(
        self,
        dataset: pl.DataFrame,
        embedding_dim: int,
        target_variable: str | None = None,
        batch_size: int = 32,
    ) -> None:
        super().__init__()

        self.dataset = dataset
        self.embedding_dim = embedding_dim
        self.batch_size = batch_size
        self.train_test_split: dict[int, str] | None = None
        self.target_variable = target_variable

        self._train_cache: tuple[InteractionDataset, dict[int, list[int]]] | None = None
        self._test_cache: tuple[InteractionDataset, dict[int, list[int]]] | None = None

    @staticmethod
    def _build_query_to_indices(dataset: InteractionDataset) -> dict[int, list[int]]:
        query_ids = dataset.dataset.get_column("query_id").to_list()
        mapping: dict[int, list[int]] = defaultdict(list)
        for idx, qid in enumerate(query_ids):
            mapping[qid].append(idx)
        return dict(mapping)

    def _get_train_data(self) -> tuple[InteractionDataset, dict[int, list[int]]]:
        if self._train_cache is None:
            ds = InteractionDataset(
                dataset=self.dataset.filter(pl.col("train_test_split") == 1.0),
                embedding_dim=self.embedding_dim,
                target_variable=self.target_variable,
            )
            self._train_cache = (ds, self._build_query_to_indices(ds))
        return self._train_cache

    def _get_test_data(self) -> tuple[InteractionDataset, dict[int, list[int]]]:
        if self._test_cache is None:
            ds = InteractionDataset(
                dataset=self.dataset.filter(pl.col("train_test_split") == 0.0),
                embedding_dim=self.embedding_dim,
                target_variable=self.target_variable,
            )
            self._test_cache = (ds, self._build_query_to_indices(ds))
        return self._test_cache

    @property
    def train_dataset(self) -> torch.utils.data.DataLoader:
        dataset_obj, query_to_indices = self._get_train_data()

        batch_sampler = UniqueQueryBatchSampler(
            dataset=dataset_obj,
            batch_size=self.batch_size,
            shuffle=True,
            drop_last=False,
            query_to_indices=query_to_indices,
        )

        return torch.utils.data.DataLoader(
            dataset_obj,
            batch_sampler=batch_sampler,
        )

    @property
    def test_dataset(self) -> torch.utils.data.DataLoader:
        dataset_obj, query_to_indices = self._get_test_data()

        batch_sampler = UniqueQueryBatchSampler(
            dataset=dataset_obj,
            batch_size=self.batch_size,
            shuffle=False,
            drop_last=False,
            query_to_indices=query_to_indices,
        )

        return torch.utils.data.DataLoader(
            dataset_obj,
            batch_sampler=batch_sampler,
        )

    @property
    def candidate_dataset(self) -> dict[str, torch.Tensor]:
        unique_candidates = self.dataset.select(
            ["candidate_id", "candidate_embedding"]
        ).unique(subset=["candidate_id"])

        return {
            "candidate_id": torch.tensor(
                unique_candidates.get_column("candidate_id").to_list(),
                dtype=torch.long,
            ),
            "candidate_embedding": torch.tensor(
                unique_candidates.get_column("candidate_embedding").to_list(),
                dtype=torch.float,
            ),
        }

    @property
    def true_hits(self) -> pl.DataFrame:
        """Returns a DataFrame with true hits for each query_id for the test set."""
        return (
            self.dataset.filter(
                pl.col("train_test_split") == 0.0,
            )
            .select(["query_id", "candidate_id"])
            .group_by("query_id")
            .agg(
                pl.col("candidate_id").alias("true_hits"),
            )
        )

    @property
    def train_positives(self) -> dict[int, list[int]]:
        """All positive candidate_ids per query_id in the training set."""
        train_df = self._get_train_data()[0].dataset
        return dict(
            train_df.select(["query_id", "candidate_id"])
            .group_by("query_id")
            .agg(pl.col("candidate_id"))
            .iter_rows()
        )
