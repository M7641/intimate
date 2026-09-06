"""Shared helpers for example scripts: data generation, training, evaluation."""

from __future__ import annotations


import torch
import torch.nn as nn
import torch.nn.functional as F
from torch import Tensor

from mana import mimic

# -- palette (ANSI) ----------------------------------------------------------

RESET = "\033[0m"
BOLD = "\033[1m"
DIM = "\033[90m"
CYAN = "\033[96m"
MAGENTA = "\033[95m"
BLUE = "\033[94m"
GREEN = "\033[92m"
YELLOW = "\033[93m"
RED = "\033[91m"


def _kv(key: str, value: str, *, val_color: str = CYAN, suffix: str = "") -> None:
    line = f"{BLUE}{BOLD}  {key:7s}  {RESET}{val_color}{value}{RESET}"
    if suffix:
        line += f"  {DIM}{suffix}{RESET}"
    print(line)


def header(name: str) -> None:
    torch.manual_seed(42)
    print()
    print(f"  {MAGENTA}{BOLD}mimic — {name}{RESET}")
    print(f"  {DIM}─────────────────────────────────────{RESET}")


# -- data ---------------------------------------------------------------------


def make_clustered_data(
    clusters: int = 5,
    samples_per_cluster: int = 80,
    feature_dim: int = 10,
    batch_size: int = 128,
) -> tuple[mimic.SetDataset, object, list[int], int]:
    """Synthetic clustered set data. Returns (dataset, dataloader, labels, total)."""
    centers = torch.eye(feature_dim)[:clusters] * 2.0
    all_sets: list[Tensor] = []
    labels: list[int] = []

    for cid in range(clusters):
        for _ in range(samples_per_cluster):
            set_size = int(torch.randint(5, 16, (1,)).item())
            elements = centers[cid] + torch.randn(set_size, feature_dim) * 0.5
            all_sets.append(elements)
            labels.append(cid)

    total = len(all_sets)
    sizes = [s.shape[0] for s in all_sets]
    _kv(
        "data",
        f"{total} sets, {clusters} clusters, sizes {min(sizes)}-{max(sizes)}, dim={feature_dim}",
    )

    dataset = mimic.SetDataset(all_sets)
    dataloader = mimic.set_dataloader(dataset, batch_size=batch_size, shuffle=True)
    return dataset, dataloader, labels, total


# -- training -----------------------------------------------------------------


def train(
    name: str,
    model: nn.Module,
    dataloader,
    *,
    epochs: int = 50,
    scheduler: str = "cosine",
    warmup_epochs: int = 0,
) -> nn.Module:
    """Train model, print result summary. Returns the model."""
    print()
    print(f"  {MAGENTA}{BOLD}▸ {name}{RESET}")

    parts = [f"{epochs} epochs", f"sched={scheduler}"]
    if warmup_epochs > 0:
        parts.append(f"warmup={warmup_epochs}")
    _kv("config", ", ".join(parts))
    print()

    config = mimic.TrainConfig(
        epochs=epochs,
        warmup_epochs=warmup_epochs,
        scheduler=scheduler,
    )
    result = mimic.ContrastiveTrainer(model, config).fit(dataloader)

    delta = result.epoch_losses[0] - result.final_loss
    print()
    print(
        f"  {BLUE}{BOLD}result {RESET}"
        f"{YELLOW}{result.epoch_losses[0]:.3f}{RESET}"
        f" {DIM}->{RESET} "
        f"{GREEN}{BOLD}{result.final_loss:.3f}{RESET}"
        f"  {DIM}(Δ {delta:+.3f}){RESET}"
    )
    return model


# -- evaluation ---------------------------------------------------------------


def evaluate(
    model: nn.Module,
    dataset: mimic.SetDataset,
    labels: list[int],
    clusters: int,
    total: int,
) -> None:
    """Evaluate embedding quality and print metrics."""
    print()
    print(f"  {MAGENTA}{BOLD}▸ Embedding Quality{RESET}")

    eval_loader = mimic.set_dataloader(dataset, batch_size=total, shuffle=False)
    embeddings = model.encode_all(eval_loader)
    labels_t = torch.tensor(labels)

    _kv("shape", str(list(embeddings.shape)))

    knn_acc = mimic.knn_accuracy(embeddings, labels_t, embeddings, labels_t, k=10)
    _kv(
        "kNN@10",
        f"{knn_acc:.1%}",
        val_color=GREEN if knn_acc > 0.8 else YELLOW,
        suffix="accuracy",
    )

    sil = mimic.silhouette(embeddings, labels_t)
    _kv(
        "silhou",
        f"{sil:.3f}",
        val_color=GREEN if sil > 0.3 else YELLOW,
        suffix="silhouette score [-1, 1]",
    )

    uni = mimic.uniformity(embeddings)
    _kv(
        "unifrm",
        f"{uni:.3f}",
        val_color=CYAN,
        suffix="uniformity (lower = more spread)",
    )

    embeddings_norm = F.normalize(embeddings, dim=-1)
    sim = embeddings_norm @ embeddings_norm.T

    within_sims: list[float] = []
    across_sims: list[float] = []
    for i in range(clusters):
        mask_i = labels_t == i
        block = sim[mask_i][:, mask_i]
        n = block.shape[0]
        within_sims.append(block[~torch.eye(n, dtype=torch.bool)].mean().item())
        for j in range(i + 1, clusters):
            mask_j = labels_t == j
            across_sims.append(sim[mask_i][:, mask_j].mean().item())

    avg_within = sum(within_sims) / len(within_sims)
    avg_across = sum(across_sims) / len(across_sims)
    gap = avg_within - avg_across

    _kv(
        "within",
        f"{avg_within:.3f}",
        val_color=GREEN,
        suffix="avg cosine sim (same cluster)",
    )
    _kv(
        "across",
        f"{avg_across:.3f}",
        val_color=YELLOW,
        suffix="avg cosine sim (diff cluster)",
    )
    _kv("gap", f"{gap:.3f}", val_color=CYAN, suffix="separation (within - across)")

    print()
    if avg_within > avg_across:
        print(
            f"  {GREEN}{BOLD}✔ Same-cluster objects are more similar — contrastive learning works!{RESET}"
        )
    else:
        print(
            f"  {YELLOW}{BOLD}✘ Clusters not yet separated — try more epochs or tuning.{RESET}"
        )
    print()
