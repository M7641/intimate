"""End-to-end: synthetic data → mimic embeddings → regression.

Demonstrates the full mana pipeline:
  1. Generate entities with variable-size feature sets and a continuous target.
  2. Train mimic to learn fixed-size embeddings per entity.
  3. Store embeddings in an EmbeddingStore.
  4. Train a linear regressor on the embeddings to predict the target.
  5. Evaluate with R² and RMSE on a held-out test set.

Usage:
    uv run mana examples embedding-regression
"""

from __future__ import annotations


import torch
import torch.nn as nn
from torch import Tensor

from mana import mimic
from mana.embeddings import EmbeddingStore

from ._common import BOLD, DIM, GREEN, MAGENTA, RED, RESET, YELLOW, _kv, header

# ---------------------------------------------------------------------------
# 1. Data generation
# ---------------------------------------------------------------------------

NUM_ENTITIES = 600
FEATURE_DIM = 8
NUM_CLUSTERS = 5
TEST_RATIO = 0.2


def _generate_data(
    seed: int = 42,
) -> tuple[list[Tensor], Tensor, Tensor]:
    """Synthetic entities with variable-size sets and a structure-dependent target.

    Each entity belongs to a hidden cluster. The regression target depends on
    BOTH the cluster identity AND set-level structure (within-set spread and
    set size). Mean-pooling captures the cluster component but loses the
    structural signal — mimic embeddings can learn to preserve both.

    Returns:
        sets: list of (N_i, D) tensors — one per entity.
        targets: (num_entities,) float targets.
        cluster_ids: (num_entities,) int cluster labels (for diagnostics).
    """
    rng = torch.Generator().manual_seed(seed)

    # Cluster centres and per-cluster target weights
    centres = torch.randn(NUM_CLUSTERS, FEATURE_DIM, generator=rng) * 2.0
    target_weights = torch.randn(FEATURE_DIM, generator=rng)

    sets: list[Tensor] = []
    targets: list[float] = []
    cluster_ids: list[int] = []

    for i in range(NUM_ENTITIES):
        cid = i % NUM_CLUSTERS
        set_size = torch.randint(5, 25, (1,), generator=rng).item()

        # Vary the within-set spread per entity (some tight, some diffuse)
        entity_spread = 0.3 + torch.rand(1, generator=rng).item() * 1.2
        elements = (
            centres[cid]
            + torch.randn(set_size, FEATURE_DIM, generator=rng) * entity_spread
        )
        sets.append(elements)

        # Target depends on three things mean-pooling partially misses:
        #   1. cluster identity (mean-pool gets this)
        #   2. within-set diversity (mean-pool loses this)
        #   3. set size effect (mean-pool loses this)
        cluster_component = (centres[cid] @ target_weights).item()
        spread_component = elements.std(dim=0).mean().item()  # within-set diversity
        size_component = torch.tensor(float(set_size)).log().item()  # log(set_size)

        target = cluster_component + 3.0 * spread_component * size_component
        noise = torch.randn(1, generator=rng).item() * 0.3
        targets.append(target + noise)
        cluster_ids.append(cid)

    return sets, torch.tensor(targets), torch.tensor(cluster_ids)


# ---------------------------------------------------------------------------
# 2. Simple linear regressor (self-contained, no external deps)
# ---------------------------------------------------------------------------


class LinearRegressor(nn.Module):
    """One hidden layer regressor: embedding_dim → 1."""

    def __init__(self, input_dim: int, hidden_dim: int = 32) -> None:
        super().__init__()
        self.net = nn.Sequential(
            nn.Linear(input_dim, hidden_dim),
            nn.ReLU(),
            nn.Linear(hidden_dim, 1),
        )

    def forward(self, x: Tensor) -> Tensor:
        return self.net(x).squeeze(-1)


def _train_regressor(
    model: LinearRegressor,
    X_train: Tensor,
    y_train: Tensor,
    epochs: int = 200,
    lr: float = 1e-3,
) -> list[float]:
    """Train the regressor with MSE loss. Returns per-epoch losses."""
    optimiser = torch.optim.Adam(model.parameters(), lr=lr)
    loss_fn = nn.MSELoss()
    losses: list[float] = []

    for _ in range(epochs):
        model.train()
        pred = model(X_train)
        loss = loss_fn(pred, y_train)
        loss.backward()
        optimiser.step()
        optimiser.zero_grad()
        losses.append(loss.item())

    return losses


@torch.no_grad()
def _evaluate_regressor(
    model: LinearRegressor,
    X: Tensor,
    y: Tensor,
) -> tuple[float, float]:
    """Returns (R², RMSE)."""
    model.eval()
    pred = model(X)
    ss_res = ((y - pred) ** 2).sum()
    ss_tot = ((y - y.mean()) ** 2).sum()
    r2 = 1.0 - (ss_res / ss_tot).item()
    rmse = ss_res.div(len(y)).sqrt().item()
    return r2, rmse


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------


def main() -> None:
    header("Embedding → Regression")

    # -- 1. generate data --
    sets, targets, cluster_ids = _generate_data()
    sizes = [s.shape[0] for s in sets]
    _kv("data", f"{NUM_ENTITIES} entities, {NUM_CLUSTERS} clusters")
    _kv("sets", f"sizes {min(sizes)}–{max(sizes)}, dim={FEATURE_DIM}")
    _kv("target", f"range [{targets.min():.1f}, {targets.max():.1f}]")

    # -- 2. train mimic embeddings --
    print()
    print(f"  {MAGENTA}{BOLD}▸ Step 1: Learn embeddings (mimic){RESET}")

    dataset = mimic.SetDataset(sets, ids=list(range(NUM_ENTITIES)))
    loader = mimic.set_dataloader(dataset, batch_size=256, shuffle=True)

    enc_dim = 64
    model = mimic.MimicModel(
        # Set Transformer: attention captures inter-element relationships
        # (e.g. within-set spread, pairwise structure) that mean-pooling loses.
        encoder=mimic.SetTransformerEncoder(
            input_dim=FEATURE_DIM,
            dim=64,
            output_dim=enc_dim,
            num_heads=4,
            num_layers=2,
            dropout=0.1,
        ),
        projector=mimic.MLPProjector(enc_dim, 64, 32),
        augmentation=mimic.Compose(
            [
                mimic.SubsetSample(keep_fraction=0.7),
                mimic.FeatureNoise(std=0.1),
                mimic.FeatureMask(p=0.15),
            ]
        ),
        loss_fn=mimic.NTXentLoss(temperature=0.1, learnable=True, strategy="debiased"),
        variance_weight=0.04,
        covariance_weight=0.04,
        # Reconstruction loss preserves information about the input,
        # not just discriminative features — critical for downstream regression.
        decoder=mimic.make_decoder(enc_dim, 64, FEATURE_DIM),
        reconstruction_weight=0.5,
    )

    config = mimic.TrainConfig(epochs=300, lr=1e-3, scheduler="cosine", verbose=False)
    result = mimic.ContrastiveTrainer(model, config).fit(loader)

    first, final = result.epoch_losses[0], result.final_loss
    color = GREEN if final < first else YELLOW
    _kv("loss", f"{first:.3f} → {final:.3f}", val_color=color)

    # -- 3. create EmbeddingStore --
    store = EmbeddingStore.from_mimic(model, dataset)
    _kv("store", f"{len(store)} entities × {store.dim}d")

    # -- 4. train/test split --
    print()
    print(f"  {MAGENTA}{BOLD}▸ Step 2: Regression on embeddings{RESET}")

    train_store, test_store = store.split(test_ratio=TEST_RATIO, seed=7)

    # Align targets with store splits
    train_targets = targets[torch.tensor([store.ids.index(i) for i in train_store.ids])]
    test_targets = targets[torch.tensor([store.ids.index(i) for i in test_store.ids])]

    X_train = train_store.embeddings
    y_train = train_targets
    X_test = test_store.embeddings
    y_test = test_targets

    _kv("split", f"train={len(train_store)}, test={len(test_store)}")

    # -- 5. train regressor --
    regressor = LinearRegressor(input_dim=store.dim)
    reg_losses = _train_regressor(regressor, X_train, y_train, epochs=200)
    _kv("reg loss", f"{reg_losses[0]:.3f} → {reg_losses[-1]:.3f}")

    # -- 6. evaluate --
    print()
    print(f"  {MAGENTA}{BOLD}▸ Step 3: Evaluate{RESET}")

    r2_train, rmse_train = _evaluate_regressor(regressor, X_train, y_train)
    r2_test, rmse_test = _evaluate_regressor(regressor, X_test, y_test)

    _kv("train", f"R²={r2_train:.3f}  RMSE={rmse_train:.3f}", val_color=DIM)

    test_color = GREEN if r2_test > 0.5 else YELLOW if r2_test > 0.0 else RED
    _kv("test", f"R²={r2_test:.3f}  RMSE={rmse_test:.3f}", val_color=test_color)

    # -- baseline comparison --
    print()
    print(f"  {MAGENTA}{BOLD}▸ Baseline: mean-pooled features (no mimic){RESET}")

    raw_means = torch.stack([s.mean(dim=0) for s in sets])
    train_raw_idx = torch.tensor([store.ids.index(i) for i in train_store.ids])
    test_raw_idx = torch.tensor([store.ids.index(i) for i in test_store.ids])
    X_train_raw = raw_means[train_raw_idx]
    X_test_raw = raw_means[test_raw_idx]

    baseline = LinearRegressor(input_dim=FEATURE_DIM)
    _train_regressor(baseline, X_train_raw, y_train, epochs=200)

    r2_base_train, rmse_base_train = _evaluate_regressor(baseline, X_train_raw, y_train)
    r2_base_test, rmse_base_test = _evaluate_regressor(baseline, X_test_raw, y_test)

    _kv("train", f"R²={r2_base_train:.3f}  RMSE={rmse_base_train:.3f}", val_color=DIM)
    base_color = GREEN if r2_base_test > 0.5 else YELLOW if r2_base_test > 0.0 else RED
    _kv(
        "test",
        f"R²={r2_base_test:.3f}  RMSE={rmse_base_test:.3f}",
        val_color=base_color,
    )

    # -- verdict --
    print()
    delta = r2_test - r2_base_test
    if delta > 0.01:
        print(
            f"  {GREEN}{BOLD}✔ Mimic embeddings outperform raw mean-pooling "
            f"by R² Δ={delta:+.3f}{RESET}"
        )
    elif delta > -0.01:
        print(
            f"  {YELLOW}{BOLD}≈ Roughly on par with mean-pooling "
            f"(R² Δ={delta:+.3f}){RESET}"
        )
    else:
        print(
            f"  {YELLOW}{BOLD}✘ Mean-pooling won this round — "
            f"try more mimic epochs or tuning (R² Δ={delta:+.3f}){RESET}"
        )
    print()


if __name__ == "__main__":
    main()
