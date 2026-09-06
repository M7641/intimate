"""Polars DataFrame with mixed categorical + continuous features.

Scenario — customer transaction embeddings:
  Each customer has a variable number of transactions. Each transaction has
  categorical columns (merchant category, payment method, channel) and
  continuous columns (amount, hour of day). We learn a fixed-size embedding
  per customer using contrastive learning on these variable-size sets.

Usage:
    uv run mana examples polars-tabular
"""

from __future__ import annotations

import random
from pathlib import Path

import polars as pl
import torch
import torch.nn as nn

from mana import mimic
from mana.embeddings import EmbeddingStore

from ._common import BOLD, DIM, GREEN, MAGENTA, RESET, YELLOW, _kv, header

MERCHANTS = ["grocery", "gas", "restaurant", "online", "travel", "pharmacy"]
PAYMENTS = ["card", "cash", "mobile"]
CHANNELS = ["in_store", "app", "web"]


def _make_transactions(n_customers: int = 200, seed: int = 42) -> pl.DataFrame:
    """Generate a realistic-ish transaction DataFrame."""
    rng = random.Random(seed)
    rows: list[dict] = []
    for cid in range(n_customers):
        n_txn = rng.randint(10, 15)
        for _ in range(n_txn):
            rows.append(
                {
                    "customer_id": cid,
                    "merchant": rng.choice(MERCHANTS),
                    "payment": rng.choice(PAYMENTS),
                    "channel": rng.choice(CHANNELS),
                    "amount": round(rng.gauss(50, 30), 2),
                    "hour": round(rng.gauss(14, 4), 1),
                }
            )
    return pl.DataFrame(rows)


# ---------------------------------------------------------------------------
# 2. Encode & convert
# ---------------------------------------------------------------------------


def _encode(
    df: pl.DataFrame,
    cat_columns: list[str],
    cont_columns: list[str],
) -> tuple[pl.DataFrame, list[int], dict[str, dict[str, int]]]:
    """String → int categories, z-score continuous columns."""
    cat_mappings: dict[str, dict[str, int]] = {}
    for col in cat_columns:
        unique_vals = sorted(df[col].unique().to_list())
        cat_mappings[col] = {v: i for i, v in enumerate(unique_vals)}

    df_enc = df.with_columns(
        [df[col].replace_strict(cat_mappings[col]).alias(col) for col in cat_columns]
    ).with_columns(
        [(pl.col(c) - pl.col(c).mean()) / pl.col(c).std() for c in cont_columns]
    )

    cardinalities = [len(cat_mappings[col]) for col in cat_columns]
    return df_enc, cardinalities, cat_mappings


def _to_sets(
    df: pl.DataFrame,
    feature_columns: list[str],
) -> tuple[list[torch.Tensor], list[int]]:
    """Group by customer_id → list of (N_i, D) tensors."""
    sets: list[torch.Tensor] = []
    customer_ids: list[int] = []
    for cid, group in df.group_by("customer_id", maintain_order=True):
        rows = group.select(feature_columns).to_numpy()
        sets.append(torch.tensor(rows, dtype=torch.float32))
        customer_ids.append(cid[0])  # type: ignore[arg-type]
    return sets, customer_ids


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------


def main() -> None:
    header("Polars Tabular")

    cat_columns = ["merchant", "payment", "channel"]
    cont_columns = ["amount", "hour"]
    feature_columns = cat_columns + cont_columns

    # -- data --
    df = _make_transactions(n_customers=1000)
    _kv("rows", f"{len(df)} transactions, {df['customer_id'].n_unique()} customers")

    df_enc, cat_cardinalities, cat_mappings = _encode(df, cat_columns, cont_columns)
    for col, mapping in cat_mappings.items():
        _kv(col[:7], ", ".join(f"{k}→{v}" for k, v in mapping.items()), val_color=DIM)

    sets, customer_ids = _to_sets(df_enc, feature_columns)
    sizes = [s.shape[0] for s in sets]
    _kv("sets", f"sizes {min(sizes)}–{max(sizes)}, features={len(feature_columns)}")

    # -- model --
    dataset = mimic.SetDataset(sets, ids=customer_ids)
    loader = mimic.set_dataloader(dataset, batch_size=128, shuffle=True)

    enc_dim, hidden, proj_dim = 64, 64, 32

    model = mimic.MimicModel(
        encoder=mimic.DeepSetsEncoder(
            element_encoder=mimic.TabularElementEncoder(
                cat_cardinalities=cat_cardinalities,
                num_continuous=len(cont_columns),
                embedding_dim=8,
                output_dim=hidden,
            ),
            aggregator=mimic.MeanAggregator(),
            rho=nn.Sequential(nn.Linear(hidden, enc_dim), nn.ReLU()),
            output_dim=enc_dim,
        ),
        projector=mimic.MLPProjector(enc_dim, hidden, proj_dim),
        augmentation=mimic.Compose(
            [
                mimic.SubsetSample(keep_fraction=0.7),
                mimic.FeatureNoise(std=0.05),
            ]
        ),
        loss_fn=mimic.NTXentLoss(temperature=0.1),
        decoder=mimic.make_decoder(enc_dim, hidden, len(feature_columns)),
        reconstruction_weight=0.1,
    )

    # -- train (quiet, just show summary) --
    print()
    print(f"  {MAGENTA}{BOLD}▸ Training{RESET}")

    config = mimic.TrainConfig(epochs=250, lr=1e-3, scheduler="cosine", verbose=False)
    result = mimic.ContrastiveTrainer(model, config).fit(loader)

    first, final = result.epoch_losses[0], result.final_loss
    color = GREEN if final < first else YELLOW
    _kv(
        "loss",
        f"{first:.3f} → {final:.3f}",
        val_color=color,
        suffix=f"Δ {first - final:+.3f}",
    )

    # -- embeddings --
    print()
    print(f"  {MAGENTA}{BOLD}▸ Embeddings{RESET}")

    store = EmbeddingStore.from_mimic(model, dataset)
    _kv("shape", f"{len(store)} customers × {store.dim}d")

    # Show a compact preview (first 6 dims)
    preview_dims = 6
    emb_df = pl.DataFrame({"customer_id": customer_ids}).hstack(
        pl.DataFrame(
            store.embeddings[:, :preview_dims].numpy(),
            schema=[f"emb_{i}" for i in range(preview_dims)],
        )
    )

    print()
    with pl.Config(
        tbl_rows=8,
        tbl_cols=preview_dims + 2,
        float_precision=3,
        tbl_width_chars=80,
    ):
        print(emb_df)
    print(f"  {DIM}({store.dim} dims total, showing first {preview_dims}){RESET}")
    print()

    # -- save embeddings --
    out_path = Path("customer_embeddings.pt")
    store.save(out_path)
    _kv("saved", str(out_path))


if __name__ == "__main__":
    main()
