"""Retrieval recommender: synthetic embeddings → projection towers → hard negative training.

Demonstrates the retrieval pipeline:
  1. Generate synthetic query/candidate embeddings with cluster structure.
  2. Create positive interactions (same-cluster pairs).
  3. Train a RetrievalModel that learns projection layers to score pairs via dot product.
  4. Evaluate precision/recall/hit-rate against a random baseline.

The model never sees raw features — only pre-computed embedding vectors.
The learnable projection layers adapt these embeddings for the retrieval task.

Usage:
    uv run mana examples retrieval-recommender
"""

from __future__ import annotations

import tempfile

import polars as pl
import torch

from ._common import BOLD, GREEN, MAGENTA, RED, RESET, YELLOW, _kv, header

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

N_QUERIES = 100
N_CANDIDATES = 250
EMBEDDING_DIM = 16
PROJECTION_DIM = 16
N_CLUSTERS = 5
INTERACTIONS_PER_QUERY = 20
EPOCHS = 25
BATCH_SIZE = 64


# ---------------------------------------------------------------------------
# 1. Data generation
# ---------------------------------------------------------------------------


def _generate_data(seed: int = 42) -> pl.DataFrame:
    """Synthetic query-candidate interactions with pre-computed embeddings.

    Queries and candidates each belong to one of N_CLUSTERS clusters.
    Positive interactions connect queries with candidates from the same
    cluster — the retrieval model must learn this structure from the
    embedding vectors alone.
    """
    rng = torch.Generator().manual_seed(seed)

    # Cluster centres in embedding space
    centres = torch.randn(N_CLUSTERS, EMBEDDING_DIM, generator=rng) * 2.0

    query_clusters = [i % N_CLUSTERS for i in range(N_QUERIES)]
    candidate_clusters = [i % N_CLUSTERS for i in range(N_CANDIDATES)]

    query_embeddings = torch.stack(
        [
            centres[query_clusters[i]] + torch.randn(EMBEDDING_DIM, generator=rng) * 0.5
            for i in range(N_QUERIES)
        ]
    )
    candidate_embeddings = torch.stack(
        [
            centres[candidate_clusters[i]]
            + torch.randn(EMBEDDING_DIM, generator=rng) * 0.5
            for i in range(N_CANDIDATES)
        ]
    )

    # Group candidates by cluster for interaction generation
    cluster_to_candidates: dict[int, list[int]] = {}
    for cid, cc in enumerate(candidate_clusters):
        cluster_to_candidates.setdefault(cc, []).append(cid)

    # Build interactions: each query pairs with random same-cluster candidates
    query_ids: list[int] = []
    cand_ids: list[int] = []
    q_embs: list[list[float]] = []
    c_embs: list[list[float]] = []
    splits: list[float] = []

    for qid in range(N_QUERIES):
        pool = cluster_to_candidates[query_clusters[qid]]
        n_pick = min(INTERACTIONS_PER_QUERY, len(pool))
        perm = torch.randperm(len(pool), generator=rng)[:n_pick]
        chosen = [pool[p] for p in perm.tolist()]

        for idx, cid in enumerate(chosen):
            query_ids.append(qid)
            cand_ids.append(cid)
            q_embs.append(query_embeddings[qid].tolist())
            c_embs.append(candidate_embeddings[cid].tolist())
            # First interaction per query → test, rest → train
            splits.append(0.0 if idx == 0 else 1.0)

    return pl.DataFrame(
        {
            "query_id": query_ids,
            "candidate_id": cand_ids,
            "query_embedding": q_embs,
            "candidate_embedding": c_embs,
            "train_test_split": splits,
        }
    )


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------


def main() -> None:
    header("Retrieval Recommender")

    # -- 1. generate data --
    df = _generate_data()
    n_train = df.filter(pl.col("train_test_split") == 1.0).height
    n_test = df.filter(pl.col("train_test_split") == 0.0).height
    _kv("data", f"{df.height} interactions ({n_train} train, {n_test} test)")
    _kv("embed", f"dim={EMBEDDING_DIM}, {N_QUERIES} queries, {N_CANDIDATES} candidates")
    _kv("clusters", f"{N_CLUSTERS} (ground-truth structure for the model to learn)")

    # -- 2. create data loader --
    from mana.adapters.recommender.interactions import InteractionDataLoader

    data_loader = InteractionDataLoader(
        dataset=df,
        embedding_dim=EMBEDDING_DIM,
        batch_size=BATCH_SIZE,
    )

    candidate_data = data_loader.candidate_dataset
    true_hits = data_loader.true_hits

    _kv("cands", f"{candidate_data['candidate_id'].shape[0]} unique candidates loaded")

    # -- 3. build model --
    print()
    print(f"  {MAGENTA}{BOLD}▸ Training RetrievalModel{RESET}")
    _kv(
        "arch",
        f"projection {EMBEDDING_DIM}→{PROJECTION_DIM} (2-layer MLP+ReLU per side)",
    )
    _kv("config", f"{EPOCHS} epochs, batch_size={BATCH_SIZE}, hard negatives=40")
    print()

    from mana.adapters.recommender.retrieval import RetrievalModel, RetrievalTrainConfig

    config = RetrievalTrainConfig(
        projection_dim=PROJECTION_DIM,
        number_of_hard_negatives=50,
        early_stopping_patience=EPOCHS,
    )

    model = RetrievalModel(
        candidate_data=candidate_data,
        embedding_dim=EMBEDDING_DIM,
        config=config,
        true_hits=true_hits,
        k_values=[10],
    )

    # -- 4. train --
    with tempfile.TemporaryDirectory() as tmpdir:
        model.train_loop(
            epochs=EPOCHS,
            dataset=data_loader,
            checkpoint_dir=tmpdir,
        )

    # -- 5. summary --
    print()
    print(f"  {MAGENTA}{BOLD}▸ Results{RESET}")

    history = model.metrics_history
    first_loss = history["train_loss"][0]
    final_loss = history["train_loss"][-1]
    loss_color = GREEN if final_loss < first_loss else YELLOW
    _kv("loss", f"{first_loss:.3f} → {final_loss:.3f}", val_color=loss_color)

    if history["test_metrics"]:
        final_metrics = history["test_metrics"][-1]
        baseline = history.get("random_baseline", {})

        for key, value in sorted(final_metrics.items()):
            baseline_key = f"random_{key}"
            if baseline and baseline_key in baseline:
                base_val = baseline[baseline_key]
                improvement = (
                    ((value - base_val) / base_val * 100) if base_val > 0 else 0
                )
                color = GREEN if improvement > 0 else RED
                _kv(
                    key,
                    f"{value:.3f}",
                    val_color=color,
                    suffix=f"baseline: {base_val:.3f}, {improvement:+.0f}%",
                )
            else:
                _kv(key, f"{value:.3f}")

    print()


if __name__ == "__main__":
    main()
