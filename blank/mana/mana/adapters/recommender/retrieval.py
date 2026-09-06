import dataclasses
import json
import math
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import polars as pl
import torch
from pure.logging import NimbusLogger
from rich.console import Console
from rich.progress import Progress
from torch.utils.data import DataLoader

from mana.adapters.recommender.interactions import InteractionDataLoader
from mana.adapters.recommender.metrics import precision_at_k

logger = NimbusLogger.get_logger(__name__)


@dataclass
class RetrievalTrainConfig:
    # Architecture
    projection_dim: int = 64
    hidden_dim_multiplier: float = 2.0
    num_tower_layers: int = 2
    dropout: float = 0.15

    # Optimizer
    learning_rate: float = 0.001
    weight_decay: float = 0.01
    grad_clip_max_norm: float = 1.0

    # LR Scheduler
    lr_factor: float = 0.75
    lr_patience: int = 3
    lr_min: float = 1e-6

    # Hard negatives
    number_of_hard_negatives: int = 40
    refresh_every: int = 10

    # Training loop
    early_stopping_patience: int = 5
    monitor_metric: str = "precision@10"

    # Temperature
    initial_temperature: float = 0.07
    fixed_temperature: float | None = 0.07  # None = learnable
    temperature_clamp_min: float = -2.3
    temperature_clamp_max: float = 4.6

    # Curriculum (linear ramp of hard negatives)
    curriculum_min_negatives: int | None = None  # None = disabled
    curriculum_warmup_epochs: int = 5

    # Regularisation
    uniformity_weight: float = 0.05
    variance_weight: float = 0.05
    variance_gamma: float = 1.0

    # Popularity debiasing (hard negative selection)
    popularity_debias: bool = False
    popularity_bias_strength: float = 0.5


def _build_projection_tower(
    input_dim: int,
    hidden_dim: int,
    output_dim: int,
    num_layers: int,
    dropout: float,
) -> torch.nn.Sequential:
    if num_layers == 1:
        return torch.nn.Sequential(torch.nn.Linear(input_dim, output_dim))

    if num_layers == 2:
        return torch.nn.Sequential(
            torch.nn.Linear(input_dim, hidden_dim),
            torch.nn.LayerNorm(hidden_dim),
            torch.nn.GELU(),
            torch.nn.Dropout(dropout),
            torch.nn.Linear(hidden_dim, output_dim),
        )

    layers: list[torch.nn.Module] = [
        torch.nn.Linear(input_dim, hidden_dim),
        torch.nn.LayerNorm(hidden_dim),
        torch.nn.GELU(),
        torch.nn.Dropout(dropout),
    ]
    for _ in range(num_layers - 2):
        layers.extend(
            [
                torch.nn.Linear(hidden_dim, hidden_dim),
                torch.nn.LayerNorm(hidden_dim),
                torch.nn.GELU(),
                torch.nn.Dropout(dropout),
            ]
        )
    layers.append(torch.nn.Linear(hidden_dim, output_dim))
    return torch.nn.Sequential(*layers)


class _ResidualProjectionTower(torch.nn.Module):
    def __init__(
        self, tower: torch.nn.Sequential, input_dim: int, output_dim: int
    ) -> None:
        super().__init__()
        self.tower = tower
        self.skip = torch.nn.Linear(input_dim, output_dim)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.tower(x) + self.skip(x)


class RetrievalModel(torch.nn.Module):
    def __init__(
        self,
        candidate_data: dict[str, torch.Tensor],
        embedding_dim: int,
        config: RetrievalTrainConfig | None = None,
        true_hits: pl.DataFrame | None = None,
        k_values: list[int] | None = None,
    ) -> None:
        super().__init__()

        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        self.config = config if config is not None else RetrievalTrainConfig()

        if k_values is None:
            k_values = [10]

        if any(k <= 0 for k in k_values):
            msg = f"All k_values must be positive integers, got {k_values}"
            raise ValueError(msg)

        self.k_values = k_values
        self.test_metrics: dict[str, list[float]] = {}
        init_temp = self.config.fixed_temperature or self.config.initial_temperature
        log_temp = torch.log(torch.tensor(1.0 / init_temp))
        if self.config.fixed_temperature is not None:
            self.register_buffer("log_temperature", log_temp)
        else:
            self.log_temperature = torch.nn.Parameter(log_temp)
        self.embedding_dim = embedding_dim
        self.projection_dim = self.config.projection_dim
        self.hidden_dim = int(embedding_dim * self.config.hidden_dim_multiplier)

        query_tower = _build_projection_tower(
            embedding_dim,
            self.hidden_dim,
            self.projection_dim,
            self.config.num_tower_layers,
            self.config.dropout,
        )
        candidate_tower = _build_projection_tower(
            embedding_dim,
            self.hidden_dim,
            self.projection_dim,
            self.config.num_tower_layers,
            self.config.dropout,
        )
        if self.config.num_tower_layers >= 3:
            self.query_projection = _ResidualProjectionTower(
                query_tower, embedding_dim, self.projection_dim
            )
            self.candidate_projection = _ResidualProjectionTower(
                candidate_tower, embedding_dim, self.projection_dim
            )
        else:
            self.query_projection = query_tower
            self.candidate_projection = candidate_tower

        self.candidate_data = {
            key: value.to(self.device) if isinstance(value, torch.Tensor) else value
            for key, value in candidate_data.items()
        }

        if "candidate_id" not in self.candidate_data:
            msg = "candidate_data must include 'candidate_id' key."
            raise ValueError(msg)

        # Pre-compute candidate_id to index mapping for lookup during training
        self._candidate_id_to_idx = {
            cid.item(): idx
            for idx, cid in enumerate(self.candidate_data["candidate_id"])
        }
        max_candidate_id = self.candidate_data["candidate_id"].max().item()
        self._cid_to_idx_tensor = torch.full(
            (max_candidate_id + 1,), -1, dtype=torch.long, device=self.device
        )
        for cid, idx in self._candidate_id_to_idx.items():
            self._cid_to_idx_tensor[cid] = idx

        self.true_hits = true_hits if true_hits is not None else pl.DataFrame()

        # Pre-build lookup for metrics calculation
        self._true_hits_lookup = {}
        if not self.true_hits.is_empty():
            self._true_hits_lookup = dict(
                zip(
                    self.true_hits["query_id"].to_list(),
                    self.true_hits["true_hits"].to_list(),
                    strict=False,
                ),
            )

        self._train_positives: dict[int, list[int]] = {}
        self._popularity_penalty: torch.Tensor | None = None

        self.metrics_history: dict = {
            "train_loss": [],
            "test_loss": [],
            "test_metrics": [],
            "epochs": [],
            "learning_rates": [],
        }

        self._setup_optimizer_and_schedulers()

        self.metric_callables: dict[str, Callable] = {
            "precision": precision_at_k,
        }

        self.to(self.device)

    def _setup_optimizer_and_schedulers(self) -> None:
        """Setup optimizer and learning rate schedulers."""
        self.optimizer = torch.optim.AdamW(
            self.parameters(),
            lr=self.config.learning_rate,
            weight_decay=self.config.weight_decay,
        )
        self.lr_scheduler = torch.optim.lr_scheduler.ReduceLROnPlateau(
            self.optimizer,
            mode="min",
            factor=self.config.lr_factor,
            patience=self.config.lr_patience,
            min_lr=self.config.lr_min,
        )

    def _compute_popularity_penalty(self) -> torch.Tensor | None:
        if not self.config.popularity_debias or not self._train_positives:
            return None

        num_candidates = self.candidate_data["candidate_id"].size(0)
        frequency = torch.zeros(num_candidates, device=self.device)

        for pos_cids in self._train_positives.values():
            for cid in pos_cids:
                idx = self._candidate_id_to_idx.get(cid)
                if idx is not None:
                    frequency[idx] += 1.0

        penalty = self.config.popularity_bias_strength * torch.log(frequency + 1.0)
        logger.info(
            "Popularity debiasing: strength=%.2f, max_penalty=%.3f, "
            "candidates_with_interactions=%d/%d",
            self.config.popularity_bias_strength,
            penalty.max().item(),
            (frequency > 0).sum().item(),
            num_candidates,
        )
        return penalty

    @property
    def temperature(self) -> torch.Tensor:
        return torch.exp(
            -torch.clamp(
                self.log_temperature,
                min=self.config.temperature_clamp_min,
                max=self.config.temperature_clamp_max,
            )
        )

    def _uniformity_loss(
        self, embeddings: torch.Tensor, t: float = 2.0
    ) -> torch.Tensor:
        if embeddings.size(0) < 2:
            return torch.tensor(0.0, device=embeddings.device)
        sq_pdist = torch.pdist(embeddings, p=2).pow(2)
        return sq_pdist.mul(-t).exp().mean().log()

    def _variance_loss(self, embeddings: torch.Tensor) -> torch.Tensor:
        if embeddings.size(0) < 2:
            return torch.tensor(0.0, device=embeddings.device)
        std = embeddings.std(dim=0)
        return torch.relu(self.config.variance_gamma - std).mean()

    def retrieval_loss_with_hard_negatives(
        self,
        input_data: dict[str, torch.Tensor],
        all_candidate_embeds: torch.Tensor,
        query_embeds: torch.Tensor,
        number_of_hard_negatives: int | None = None,
        query_embeds_raw: torch.Tensor | None = None,
    ) -> torch.Tensor:
        """Single-positive cross-entropy loss with hard negative mining.

        Each batch row provides one positive candidate. All other known positives
        for the same query (via _train_positives) are masked from the negative pool,
        preventing contradictory gradients across batches.

        NOTE: With low interaction volumes (3-5 per query), alternative loss
        formulations may improve learning:
        - Multi-positive supervised contrastive loss (see retrieval_loss_multi_positive)
          groups all positives per query into one step for a stronger gradient signal.
        - Bayesian Personalised Ranking (BPR) optimises pairwise ordering directly.
        - Sampled softmax with log-uniform negative sampling can reduce popularity bias.
        Revisit if the single-positive CE plateaus with more data or richer embeddings.

        Currently, we calculate all the similarities between the query embeddings
        and all candidate embeddings to find hard negatives.

        This will be the bottleneck when the candidate set is large. We can
        sample candidates to reduce the computation at the cost of accuracy.

        We can also use ANN indexes to find hard negatives more efficiently.
        This also has a cost to accuracy but will be better than random sampling.

        To make training harder, we can decrease the number of hard negatives.
        The model would then have to work harder to distinguish between the positive
        and the hard negatives as this smaller number of candidates
        are the most similar.
        """
        if number_of_hard_negatives is None:
            number_of_hard_negatives = self.config.number_of_hard_negatives

        # Select hard negatives (no gradients needed — only indices are used)
        with torch.no_grad():
            all_similarities = torch.matmul(
                query_embeds.detach(), all_candidate_embeds.T
            )

            # Get positive candidate indices for the batch
            positive_indices = self._cid_to_idx_tensor[
                input_data["candidate_id"].to(self.device)
            ]

            if (positive_indices == -1).any():
                bad_cids = input_data["candidate_id"][positive_indices == -1].tolist()
                msg = f"candidate_ids not found in candidate_data: {bad_cids}"
                raise ValueError(msg)

            batch_indices = torch.arange(query_embeds.size(0), device=self.device)

            # set the positive indices to -inf so they are not selected as hard negatives
            all_similarities[batch_indices, positive_indices] = float("-inf")

            # Mask all known training positives for each query (not just the batch row's)
            if self._train_positives:
                for i, qid in enumerate(input_data["query_id"]):
                    pos_cids = self._train_positives.get(qid.item(), [])
                    pos_idxs = [
                        self._candidate_id_to_idx[c]
                        for c in pos_cids
                        if c in self._candidate_id_to_idx
                    ]
                    if pos_idxs:
                        all_similarities[i, pos_idxs] = float("-inf")

            if self._popularity_penalty is not None:
                all_similarities = (
                    all_similarities - self._popularity_penalty.unsqueeze(0)
                )

            _, hard_negative_indices = torch.topk(
                all_similarities,
                k=min(number_of_hard_negatives, all_candidate_embeds.size(0) - 1),
                dim=1,
                largest=True,
            )

        # Recompute embeddings for selected candidates with gradients
        batch_size = query_embeds.size(0)
        num_negatives = hard_negative_indices.size(1)

        selected_candidate_indices = torch.cat(
            [
                positive_indices.unsqueeze(1),
                hard_negative_indices,
            ],
            dim=1,
        )

        # Gather raw embeddings and project with gradients
        flat_indices = selected_candidate_indices.flatten()
        selected_raw = self.candidate_data["candidate_embedding"][flat_indices]
        selected_embeds = self.candidate_projection(selected_raw)
        selected_embeds = torch.nn.functional.normalize(selected_embeds, p=2, dim=1)

        # Reshape to (batch_size, 1 + num_negatives, projection_dim)
        selected_embeds = selected_embeds.view(
            batch_size, 1 + num_negatives, self.projection_dim
        )

        scores = (
            torch.matmul(
                query_embeds.unsqueeze(1),
                selected_embeds.transpose(1, 2),
            ).squeeze(1)
            / self.temperature
        )

        # Create labels: positive is always at index 0, rest are negatives
        labels = torch.zeros(scores.size(0), dtype=torch.long, device=scores.device)
        ce_loss = torch.nn.functional.cross_entropy(scores, labels, reduction="mean")

        if self.config.uniformity_weight > 0:
            ce_loss = ce_loss + self.config.uniformity_weight * self._uniformity_loss(
                query_embeds
            )

        if self.config.variance_weight > 0 and query_embeds_raw is not None:
            ce_loss = ce_loss + self.config.variance_weight * self._variance_loss(
                query_embeds_raw
            )

        return ce_loss

    def retrieval_loss_multi_positive(
        self,
        input_data: dict[str, torch.Tensor],
        all_candidate_embeds: torch.Tensor,
        query_embeds: torch.Tensor,
        number_of_hard_negatives: int | None = None,
        query_embeds_raw: torch.Tensor | None = None,
    ) -> torch.Tensor:
        """Supervised contrastive loss using ALL positives per query."""
        if number_of_hard_negatives is None:
            number_of_hard_negatives = self.config.number_of_hard_negatives

        # Deduplicate queries in the batch
        batch_query_ids = input_data["query_id"].tolist()
        seen: dict[int, int] = {}
        unique_indices: list[int] = []
        for i, qid in enumerate(batch_query_ids):
            if qid not in seen:
                seen[qid] = len(unique_indices)
                unique_indices.append(i)

        unique_query_ids = [batch_query_ids[i] for i in unique_indices]
        num_unique = len(unique_query_ids)
        query_embeds_unique = query_embeds[unique_indices]

        # Gather all positives per query from _train_positives
        all_pos_indices: list[list[int]] = []
        valid_query_indices: list[int] = []
        for idx, qid in enumerate(unique_query_ids):
            pos_cids = self._train_positives.get(qid, [])
            pos_idxs = [
                self._candidate_id_to_idx[c]
                for c in pos_cids
                if c in self._candidate_id_to_idx
            ]
            if not pos_idxs:
                cid = input_data["candidate_id"][unique_indices[idx]].item()
                if cid in self._candidate_id_to_idx:
                    pos_idxs = [self._candidate_id_to_idx[cid]]
            all_pos_indices.append(pos_idxs)
            if pos_idxs:
                valid_query_indices.append(idx)

        # Filter to only queries with at least one positive
        if not valid_query_indices:
            return torch.tensor(0.0, device=self.device, requires_grad=True)

        if len(valid_query_indices) < num_unique:
            logger.warning(
                "Skipping %d/%d queries with no positives in batch",
                num_unique - len(valid_query_indices),
                num_unique,
            )
            all_pos_indices = [all_pos_indices[i] for i in valid_query_indices]
            unique_indices = [unique_indices[i] for i in valid_query_indices]
            num_unique = len(valid_query_indices)
            query_embeds_unique = query_embeds[unique_indices]

        max_k = max(len(p) for p in all_pos_indices)

        # Pad positives to (num_unique, max_k) — fill padding with first positive
        padded_pos = torch.zeros(
            num_unique, max_k, dtype=torch.long, device=self.device
        )
        pos_mask = torch.zeros(num_unique, max_k, dtype=torch.bool, device=self.device)
        for i, pos_idxs in enumerate(all_pos_indices):
            k_i = len(pos_idxs)
            pos_tensor = torch.tensor(pos_idxs, dtype=torch.long, device=self.device)
            padded_pos[i, :k_i] = pos_tensor
            padded_pos[i, k_i:] = pos_idxs[0]
            pos_mask[i, :k_i] = True

        # Mine hard negatives (no gradients)
        with torch.no_grad():
            sims = torch.matmul(query_embeds_unique.detach(), all_candidate_embeds.T)

            for i, pos_idxs in enumerate(all_pos_indices):
                sims[i, pos_idxs] = float("-inf")

            if self._popularity_penalty is not None:
                sims = sims - self._popularity_penalty.unsqueeze(0)

            k_neg = min(
                number_of_hard_negatives,
                all_candidate_embeds.size(0) - max_k,
            )
            _, hard_neg_indices = torch.topk(sims, k=k_neg, dim=1, largest=True)

        # Build candidate set: [positives | negatives]
        selected = torch.cat([padded_pos, hard_neg_indices], dim=1)
        flat_indices = selected.flatten()

        selected_raw = self.candidate_data["candidate_embedding"][flat_indices]
        selected_embeds = self.candidate_projection(selected_raw)
        selected_embeds = torch.nn.functional.normalize(selected_embeds, p=2, dim=1)
        selected_embeds = selected_embeds.view(
            num_unique, max_k + k_neg, self.projection_dim
        )

        # Scores — no in-place modifications
        scores = (
            torch.matmul(
                query_embeds_unique.unsqueeze(1),
                selected_embeds.transpose(1, 2),
            ).squeeze(1)
            / self.temperature
        )

        # Build a full mask: True = exclude from logsumexp
        exclude_mask = torch.zeros_like(scores, dtype=torch.bool)
        exclude_mask[:, :max_k] = ~pos_mask
        masked_scores = scores.masked_fill(exclude_mask, float("-inf"))

        # Supervised contrastive loss
        log_denom = torch.logsumexp(masked_scores, dim=1, keepdim=True)
        # log_prob for each positive = score - log_denom
        # Only extract real positive positions using torch.where to avoid -inf * 0
        pos_scores = masked_scores[:, :max_k]
        pos_log_probs = torch.where(
            pos_mask,
            pos_scores - log_denom,
            torch.zeros_like(pos_scores),
        )
        num_pos = pos_mask.sum(dim=1).float().clamp(min=1)
        per_query_loss = -pos_log_probs.sum(dim=1) / num_pos
        loss = per_query_loss.mean()

        # Regularisation
        if self.config.uniformity_weight > 0:
            loss = loss + self.config.uniformity_weight * self._uniformity_loss(
                query_embeds_unique
            )

        if self.config.variance_weight > 0 and query_embeds_raw is not None:
            raw_unique = query_embeds_raw[unique_indices]
            loss = loss + self.config.variance_weight * self._variance_loss(raw_unique)

        return loss

    def _compute_candidate_embeds(self) -> torch.Tensor:
        """Compute L2-normalised candidate embeddings without gradients."""
        with torch.no_grad():
            embeds = self.candidate_projection(
                self.candidate_data["candidate_embedding"]
            )
            return torch.nn.functional.normalize(embeds, p=2, dim=1)

    def train_model(
        self,
        data_loader: DataLoader,
        console: Console,
        number_of_hard_negatives: int | None = None,
    ) -> float:
        """Train the model for one epoch using hard negative mining."""
        self.train()

        total_loss = 0
        batch_count = 0
        all_candidate_embeds = self._compute_candidate_embeds()
        refresh_every = self.config.refresh_every

        for x in data_loader:
            if batch_count > 0 and batch_count % refresh_every == 0:
                all_candidate_embeds = self._compute_candidate_embeds()

            self.optimizer.zero_grad()

            query_embeds_raw = self.query_projection(
                x["query_embedding"].to(self.device)
            )
            query_embeds = torch.nn.functional.normalize(query_embeds_raw, p=2, dim=1)

            loss = self.retrieval_loss_with_hard_negatives(
                input_data=x,
                query_embeds=query_embeds,
                all_candidate_embeds=all_candidate_embeds,
                number_of_hard_negatives=number_of_hard_negatives,
                query_embeds_raw=query_embeds_raw,
            )

            if torch.isnan(loss) or torch.isinf(loss):
                logger.error(
                    "NaN/Inf loss at batch %d. "
                    "query_embeds has_nan=%s, all_candidate_embeds has_nan=%s, "
                    "temperature=%.6f, batch_size=%d",
                    batch_count,
                    torch.isnan(query_embeds).any().item(),
                    torch.isnan(all_candidate_embeds).any().item(),
                    self.temperature.item(),
                    query_embeds.size(0),
                )
                msg = f"NaN/Inf loss detected at batch {batch_count}"
                raise RuntimeError(msg)

            total_loss += loss.item()
            loss.backward()
            torch.nn.utils.clip_grad_norm_(
                self.parameters(), max_norm=self.config.grad_clip_max_norm
            )
            self.optimizer.step()
            batch_count += 1

        avg_loss = total_loss / batch_count if batch_count > 0 else 0.0
        console.log(f"Train Loss: {avg_loss:>7f}")
        return avg_loss

    def test_model(
        self,
        data_loader: DataLoader,
        console: Console,
        epoch: int,
        random_baseline: dict[str, float] | None = None,
    ) -> float:
        """Evaluate the model on the test dataset with loss and metrics calculation."""

        self.eval()
        total_loss = 0
        batch_count = 0
        full_test_metrics = {}

        with torch.no_grad():
            all_candidate_embeds = self._compute_candidate_embeds()

            for x in data_loader:
                batch_count += 1
                query_embeds = self.query_projection(
                    x["query_embedding"].to(self.device)
                )
                query_embeds = torch.nn.functional.normalize(query_embeds, p=2, dim=1)

                loss = self.retrieval_loss_with_hard_negatives(
                    input_data=x,
                    query_embeds=query_embeds,
                    all_candidate_embeds=all_candidate_embeds,
                )
                total_loss += loss.item()

                test_metrics = self.calculate_test_metrics(
                    input_data=x,
                    query_embeddings=query_embeds,
                    all_candidate_embeddings=all_candidate_embeds,
                )

                # Accumulate metrics across batches
                for metric_name, values in test_metrics.items():
                    if metric_name not in full_test_metrics:
                        full_test_metrics[metric_name] = []
                    full_test_metrics[metric_name].extend(values)

            avg_loss = total_loss / batch_count if batch_count > 0 else 0.0
            console.log(f"Test Loss: {avg_loss:>4f}")

        build_log_message = f"Epoch {epoch} Metrics"
        for key, value in full_test_metrics.items():
            average_value = sum(value) / len(value) if len(value) > 0 else 0.0
            if random_baseline and f"random_{key}" in random_baseline:
                baseline_value = random_baseline[f"random_{key}"]
                improvement = (
                    ((average_value - baseline_value) / baseline_value * 100)
                    if baseline_value > 0
                    else 0
                )
                build_log_message += f", {key}: {average_value:>4f} (baseline: {baseline_value:.4f}, +{improvement:.1f}%)"
            else:
                build_log_message += f", {key}: {average_value:>4f}"

        console.log(build_log_message)

        # Store metrics for access by train_loop
        self.test_metrics = full_test_metrics

        return avg_loss

    def train_loop(
        self,
        epochs: int,
        dataset: InteractionDataLoader,
        metrics_file_path: str | None = None,
        checkpoint_dir: str | Path = Path.cwd() / "checkpoints/",
    ) -> None:
        """Execute the complete training loop with early stopping and checkpointing."""

        early_stopping_patience = self.config.early_stopping_patience
        monitor_metric = self.config.monitor_metric

        # Reset metrics history for this training run
        self.metrics_history = {
            "train_loss": [],
            "test_loss": [],
            "test_metrics": [],
            "epochs": [],
            "learning_rates": [],
            "random_baseline": {},
        }

        self._train_positives = dataset.train_positives
        self._popularity_penalty = self._compute_popularity_penalty()

        # Early stopping variables
        best_test_loss = float("inf")
        best_metric = float("-inf")
        epochs_without_improvement = 0
        best_epoch = 0

        if isinstance(checkpoint_dir, str):
            checkpoint_dir = Path(checkpoint_dir)

        random_baseline = self.calculate_random_baseline_metrics(dataset.test_dataset)
        self.metrics_history["random_baseline"] = random_baseline

        training_start = time.time()
        with Progress() as progress:
            task = progress.add_task("[green]Training...", total=epochs)

            # Epoch 0: evaluate untrained model
            progress.console.log("Epoch 0 — untrained model baseline")
            self.test_metrics = {}
            self.test_model(
                dataset.test_dataset,
                console=progress.console,
                epoch=0,
                random_baseline=random_baseline,
            )

            for epoch in range(1, epochs + 1):
                # Curriculum: linearly ramp hard negatives over warmup epochs
                if self.config.curriculum_min_negatives is not None:
                    progress_frac = min(
                        epoch / self.config.curriculum_warmup_epochs, 1.0
                    )
                    epoch_negatives = int(
                        self.config.curriculum_min_negatives
                        + (
                            self.config.number_of_hard_negatives
                            - self.config.curriculum_min_negatives
                        )
                        * progress_frac
                    )
                else:
                    epoch_negatives = self.config.number_of_hard_negatives

                random_loss = math.log(epoch_negatives + 1)
                progress.console.log(
                    f"Epoch {epoch}/{epochs} — hard negatives: {epoch_negatives}, "
                    f"random baseline loss: {random_loss:.4f}"
                )

                train_loss = self.train_model(
                    dataset.train_dataset,
                    console=progress.console,
                    number_of_hard_negatives=epoch_negatives,
                )

                self.test_metrics = {}

                test_loss = self.test_model(
                    dataset.test_dataset,
                    console=progress.console,
                    epoch=epoch,
                    random_baseline=random_baseline,
                )

                self.metrics_history["epochs"].append(epoch)
                self.metrics_history["train_loss"].append(train_loss)
                self.metrics_history["test_loss"].append(test_loss)
                self.metrics_history["learning_rates"].append(
                    self.optimizer.param_groups[0]["lr"]
                )

                # Calculate average metrics for this epoch
                epoch_metrics = {}
                for key, values in self.test_metrics.items():
                    epoch_metrics[key] = (
                        sum(values) / len(values) if len(values) > 0 else 0.0
                    )

                self.metrics_history["test_metrics"].append(epoch_metrics)

                metric_improved = False
                monitored_value = epoch_metrics.get(monitor_metric)
                if monitored_value is not None and monitored_value > best_metric:
                    best_metric = monitored_value
                    metric_improved = True

                # Check for improvement and save best model
                if test_loss < best_test_loss or metric_improved:
                    epochs_without_improvement = 0
                    best_epoch = epoch
                    best_test_loss = test_loss
                    self.save_model(checkpoint_dir)
                    progress.console.log(
                        f"[bold green]New best model saved![/bold green] Test loss: {test_loss:.4f}, {monitor_metric}: {monitored_value:.4f}"
                        if monitored_value is not None
                        else f"[bold green]New best model saved![/bold green] Test loss: {test_loss:.4f}",
                    )
                    logger.debug(
                        "Checkpoint saved at epoch %d with test loss %.4f and %s %.4f",
                        epoch,
                        test_loss,
                        monitor_metric,
                        monitored_value
                        if monitored_value is not None
                        else float("nan"),
                    )
                else:
                    epochs_without_improvement += 1
                    progress.console.log(
                        f"No improvement for {epochs_without_improvement}/{early_stopping_patience} epochs",
                    )

                self.lr_scheduler.step(test_loss)

                # Early stopping check
                if epochs_without_improvement >= early_stopping_patience:
                    progress.console.log(
                        f"[bold yellow]Early stopping triggered![/bold yellow] No improvement for {early_stopping_patience} epochs.",
                    )
                    logger.debug(
                        "Early stopping at epoch %d. Best test loss: %.4f at epoch %d",
                        epoch,
                        best_test_loss,
                        best_epoch,
                    )
                    progress.update(task, completed=epochs)
                    break
                progress.update(task, advance=1)

        training_elapsed = time.time() - training_start
        actual_epochs = len(self.metrics_history["epochs"])
        logger.info(
            "Training loop completed in %.3fs (%.3fs avg/epoch)",
            training_elapsed,
            training_elapsed / actual_epochs,
        )

        if checkpoint_dir.exists():
            logger.debug(
                "Loading best model from epoch %d with test loss %.4f",
                best_epoch,
                best_test_loss,
            )
            loaded_model = RetrievalModel.load_model(checkpoint_dir)
            self.load_state_dict(loaded_model.state_dict())
            logger.debug("Best model restored successfully")

        if metrics_file_path is not None:
            self.save_metrics_history(metrics_file_path)

    def calculate_test_metrics(
        self,
        input_data: dict[str, torch.Tensor],
        query_embeddings: torch.Tensor,
        all_candidate_embeddings: torch.Tensor,
    ) -> dict[str, list[float]]:
        """
        Calculate metrics by evaluating queries against the full candidate corpus.

        For each query, computes similarity scores against all candidates, retrieves
        top-k candidates, and evaluates ranking metrics against ground truth.
        """
        metric_values = {}

        positive_scores = (
            torch.matmul(
                query_embeddings,
                all_candidate_embeddings.T,
            )
            / self.temperature
        )

        # Limit k to the actual number of candidates available
        num_candidates = positive_scores.size(1)
        k_to_retrieve = min(max(self.k_values), num_candidates)
        _, retrieved_indices = torch.topk(
            positive_scores,
            k=k_to_retrieve,
            dim=1,
        )

        # Evaluate at each k value
        for k in self.k_values:
            # Define metrics that depend on k
            metric_callables = {
                key + f"@{k}": lambda recs, rel, k=k, func=func: func(recs, rel, k)
                for key, func in self.metric_callables.items()
            }

            for i, query_id in enumerate(input_data["query_id"]):
                query_id_use = (
                    query_id.item() if isinstance(query_id, torch.Tensor) else query_id
                )
                relevant_items = self._true_hits_lookup.get(query_id_use)

                if relevant_items is None:
                    continue

                # Get top-k candidate IDs for this query
                recommended_candidate_indices = retrieved_indices[i, :k].cpu().numpy()
                recommended_candidate_ids = [
                    self.candidate_data["candidate_id"][idx].item()
                    for idx in recommended_candidate_indices
                ]

                # Calculate each metric
                for metric_name, metric_callable in metric_callables.items():
                    if metric_name not in metric_values:
                        metric_values[metric_name] = []

                    metric_values[metric_name].append(
                        metric_callable(recommended_candidate_ids, relevant_items),
                    )

        return metric_values

    def calculate_random_baseline_metrics(
        self,
        data_loader: DataLoader,
        num_samples: int | None = None,
    ) -> dict[str, float]:
        """
        Calculate baseline metrics by randomly selecting candidates for each query.
        This provides a benchmark to compare against trained model performance.
        """
        if num_samples is None:
            num_samples = 3

        all_candidate_ids = self.candidate_data["candidate_id"].cpu().numpy()
        num_candidates = len(all_candidate_ids)

        baseline_metrics_samples = []

        for _sample_idx in range(num_samples):
            metric_values = {}

            for x in data_loader:
                max_k = min(max(self.k_values), len(all_candidate_ids))

                for _i, query_id in enumerate(x["query_id"]):
                    query_id_use = (
                        query_id.item()
                        if isinstance(query_id, torch.Tensor)
                        else query_id
                    )
                    true_hits = self._true_hits_lookup.get(query_id_use)

                    if true_hits is None:
                        continue

                    random_candidate_indices = np.random.choice(
                        num_candidates,
                        size=max_k,
                        replace=False,
                    )
                    random_candidate_ids = [
                        all_candidate_ids[idx] for idx in random_candidate_indices
                    ]

                    # Evaluate at each k value using slices of the same candidates
                    for k in self.k_values:
                        metric_callables = {
                            "random_"
                            + key
                            + f"@{k}": lambda recs, rel, k=k, func=func: func(
                                recs, rel, k
                            )
                            for key, func in self.metric_callables.items()
                        }
                        for metric_name, metric_callable in metric_callables.items():
                            if metric_name not in metric_values:
                                metric_values[metric_name] = []

                            metric_values[metric_name].append(
                                metric_callable(random_candidate_ids[:k], true_hits),
                            )

            baseline_metrics_samples.append(metric_values)

        averaged_baseline = {}
        for metric_name in baseline_metrics_samples[0]:
            all_values = []
            for sample in baseline_metrics_samples:
                all_values.extend(sample[metric_name])
            averaged_baseline[metric_name] = (
                sum(all_values) / len(all_values) if len(all_values) > 0 else 0.0
            )

        return averaged_baseline

    def save_metrics_history(self, file_path: str | Path) -> None:
        """Save metrics history to a JSON file."""
        if isinstance(file_path, str):
            file_path = Path(file_path)

        file_path.parent.mkdir(parents=True, exist_ok=True)
        metrics_data = {
            "metadata": {
                "num_epochs": len(self.metrics_history["epochs"]),
                "k_values": self.k_values,
            },
            "history": self.metrics_history,
        }

        with file_path.open("w") as f:
            json.dump(metrics_data, f, indent=2)

    def save_model(self, file_dir: str | Path = Path.cwd() / "checkpoints/") -> None:
        """Save the complete model state to disk for later restoration."""

        if isinstance(file_dir, str):
            file_dir = Path(file_dir)

        file_dir.mkdir(parents=True, exist_ok=True)

        checkpoint = {
            "model_state_dict": self.state_dict(),
            "optimizer_state_dict": self.optimizer.state_dict(),
            "lr_scheduler_state_dict": self.lr_scheduler.state_dict(),
            "config": {
                "k_values": self.k_values,
                "embedding_dim": self.embedding_dim,
                **dataclasses.asdict(self.config),
            },
            "metrics_history": self.metrics_history,
        }

        checkpoint["candidate_data"] = {
            key: value.cpu() if isinstance(value, torch.Tensor) else value
            for key, value in self.candidate_data.items()
        }

        checkpoint["true_hits"] = self.true_hits.to_dict(as_series=False)

        torch.save(checkpoint, file_dir / "model_checkpoint.pt")
        logger.debug("Model saved to %s", file_dir / "model_checkpoint.pt")

    @classmethod
    def load_model(
        cls,
        file_dir: str | Path,
    ) -> "RetrievalModel":
        """Load a saved model checkpoint and restore complete state."""
        if isinstance(file_dir, str):
            file_dir = Path(file_dir)

        device = torch.device("cuda" if torch.cuda.is_available() else "cpu")

        checkpoint = torch.load(
            file_dir / "model_checkpoint.pt", map_location=device, weights_only=False
        )
        candidate_data = {
            key: value.to(device) if isinstance(value, torch.Tensor) else value
            for key, value in checkpoint["candidate_data"].items()
        }

        config = checkpoint["config"]
        # Extract fields that belong to RetrievalTrainConfig
        config_field_names = {f.name for f in dataclasses.fields(RetrievalTrainConfig)}
        train_config_dict = {k: v for k, v in config.items() if k in config_field_names}
        # Backward compat: old checkpoints with flat keys get defaults via dataclass
        train_config = RetrievalTrainConfig(**train_config_dict)

        model = cls(
            embedding_dim=config["embedding_dim"],
            config=train_config,
            candidate_data=candidate_data,
            true_hits=pl.DataFrame(checkpoint["true_hits"]),
            k_values=config["k_values"],
        )

        model.load_state_dict(checkpoint["model_state_dict"])
        model.optimizer.load_state_dict(checkpoint["optimizer_state_dict"])
        model.lr_scheduler.load_state_dict(checkpoint["lr_scheduler_state_dict"])

        if "metrics_history" in checkpoint:
            model.metrics_history = checkpoint["metrics_history"]

        model.to(model.device)
        logger.info("Model loaded from %s", file_dir)

        return model
