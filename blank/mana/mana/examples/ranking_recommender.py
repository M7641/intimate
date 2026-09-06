"""Ranking recommender: synthetic embeddings → projection + MLP regressor.

Demonstrates the ranking pipeline:
  1. Generate synthetic query/candidate embeddings with known relevance scores.
  2. Create interaction pairs where the score depends on embedding similarity.
  3. Train a RankingModel that projects embeddings and feeds them through an MLP.
  4. Evaluate prediction quality on held-out test pairs.

The ranking model sits downstream of retrieval — it scores a small set of
pre-filtered candidates precisely. Projections are single linear layers
(no ReLU) because the downstream regressor MLP already provides nonlinearity.

Usage:
    uv run mana examples ranking-recommender
"""

from __future__ import annotations

import tempfile

import polars as pl
import torch

from ._common import BOLD, DIM, GREEN, MAGENTA, RED, RESET, YELLOW, _kv, header

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

N_QUERIES = 150
N_CANDIDATES = 200
EMBEDDING_DIM = 32
HIDDEN_DIM = 16
PAIRS_PER_QUERY = 50
EPOCHS = 30
BATCH_SIZE = 128


# ---------------------------------------------------------------------------
# 1. Data generation
# ---------------------------------------------------------------------------


def _generate_data(seed: int = 42) -> pl.DataFrame:
    """Synthetic query-candidate pairs with relevance scores.

    Relevance is based on cosine similarity between query and candidate
    embeddings plus noise. The ranking model must learn to predict these
    scores from the raw embedding vectors via its projection + MLP.
    """
    rng = torch.Generator().manual_seed(seed)

    query_embeddings = torch.randn(N_QUERIES, EMBEDDING_DIM, generator=rng)
    candidate_embeddings = torch.randn(N_CANDIDATES, EMBEDDING_DIM, generator=rng)

    # Normalise for cosine similarity
    query_norm = torch.nn.functional.normalize(query_embeddings, dim=1)
    candidate_norm = torch.nn.functional.normalize(candidate_embeddings, dim=1)

    query_ids: list[int] = []
    cand_ids: list[int] = []
    q_embs: list[list[float]] = []
    c_embs: list[list[float]] = []
    scores: list[float] = []
    splits: list[float] = []

    for qid in range(N_QUERIES):
        # Pick random candidates for this query
        perm = torch.randperm(N_CANDIDATES, generator=rng)[:PAIRS_PER_QUERY]

        for idx, cid in enumerate(perm.tolist()):
            # Relevance = cosine similarity rescaled to [0, 1] + noise
            cos_sim = (query_norm[qid] @ candidate_norm[cid]).item()
            noise = torch.randn(1, generator=rng).item() * 0.1
            score = max(0.0, min(1.0, (cos_sim + 1.0) / 2.0 + noise))

            query_ids.append(qid)
            cand_ids.append(cid)
            q_embs.append(query_embeddings[qid].tolist())
            c_embs.append(candidate_embeddings[cid].tolist())
            scores.append(score)
            # First interaction per query goes to test, rest to train
            splits.append(0.0 if idx == 0 else 1.0)

    return pl.DataFrame(
        {
            "query_id": query_ids,
            "candidate_id": cand_ids,
            "query_embedding": q_embs,
            "candidate_embedding": c_embs,
            "score": scores,
            "train_test_split": splits,
        }
    )


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------


def main() -> None:
    header("Ranking Recommender")

    # -- 1. generate data --
    df = _generate_data()
    n_train = df.filter(pl.col("train_test_split") == 1.0).height
    n_test = df.filter(pl.col("train_test_split") == 0.0).height
    score_col = df.get_column("score")
    _kv("data", f"{df.height} pairs ({n_train} train, {n_test} test)")
    _kv("embed", f"dim={EMBEDDING_DIM}, {N_QUERIES} queries, {N_CANDIDATES} candidates")
    _kv("scores", f"range [{score_col.min():.2f}, {score_col.max():.2f}]")

    # -- 2. create data loader --
    from mana.adapters.recommender.interactions import InteractionDataLoader

    data_loader = InteractionDataLoader(
        dataset=df,
        embedding_dim=EMBEDDING_DIM,
        target_variable="score",
        batch_size=BATCH_SIZE,
    )

    # -- 3. build model --
    print()
    print(f"  {MAGENTA}{BOLD}▸ Training RankingModel{RESET}")
    _kv(
        "arch",
        f"Linear({EMBEDDING_DIM}→{HIDDEN_DIM}) per side"
        f" → MLP({HIDDEN_DIM * 2}→{HIDDEN_DIM}→1)",
    )
    _kv("config", f"{EPOCHS} epochs, batch_size={BATCH_SIZE}")
    print()

    from mana.adapters.recommender.ranking import RankingModel, RankingTrainConfig

    config = RankingTrainConfig(
        hidden_dim=HIDDEN_DIM,
        early_stopping_patience=EPOCHS,
    )
    model = RankingModel(
        embedding_dim=EMBEDDING_DIM,
        target_variable="score",
        config=config,
    )

    # -- 4. train --
    with tempfile.TemporaryDirectory() as tmpdir:
        model.train_loop(
            epochs=EPOCHS,
            dataset=data_loader,
            checkpoint_dir=tmpdir,
        )

    # -- 5. evaluate --
    print()
    print(f"  {MAGENTA}{BOLD}▸ Results{RESET}")

    history = model.metrics_history
    first_loss = history["train_loss"][0]
    final_loss = history["train_loss"][-1]
    loss_color = GREEN if final_loss < first_loss else YELLOW
    _kv("loss", f"{first_loss:.3f} → {final_loss:.3f}", val_color=loss_color)

    last_metrics = history["test_metrics"][-1]
    pearson = last_metrics.get("pearson", 0.0)
    spearman = last_metrics.get("spearman", 0.0)
    mae = last_metrics.get("mae", 0.0)

    _kv("pearson", f"{pearson:.4f}", val_color=DIM)
    _kv("spearman", f"{spearman:.4f}", val_color=DIM)
    _kv("mae", f"{mae:.4f}", val_color=DIM)

    print()
    if spearman > 0.3:
        print(
            f"  {GREEN}{BOLD}✔ Model learned meaningful ranking signal "
            f"(spearman={spearman:.3f}){RESET}"
        )
    elif spearman > 0.0:
        print(
            f"  {YELLOW}{BOLD}≈ Weak positive correlation "
            f"— try more epochs or a larger hidden_dim (spearman={spearman:.3f}){RESET}"
        )
    else:
        print(
            f"  {RED}{BOLD}✘ No ranking signal learned "
            f"— the model needs tuning (spearman={spearman:.3f}){RESET}"
        )
    print()


if __name__ == "__main__":
    main()
