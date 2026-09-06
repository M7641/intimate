import numpy as np


def precision_at_k(recommended: list, relevant: list, k: int) -> float:
    """
    Calculate precision at k.

    Args:
        recommended: List of recommended item IDs
        relevant: List of relevant (true positive) item IDs
        k: Number of top recommendations to consider

    Returns:
        Precision at k (float between 0 and 1)
    """
    if k == 0:
        return 0.0

    recommended_at_k = recommended[:k]
    relevant_set = set(relevant)

    hits = len([item for item in recommended_at_k if item in relevant_set])
    return hits / k


def recall_at_k(recommended: list, relevant: list, k: int) -> float:
    """
    Calculate recall at k.

    Args:
        recommended: List of recommended item IDs
        relevant: List of relevant (true positive) item IDs
        k: Number of top recommendations to consider

    Returns:
        Recall at k (float between 0 and 1)
    """
    if len(relevant) == 0:
        return 0.0

    recommended_at_k = recommended[:k]
    relevant_set = set(relevant)

    hits = len([item for item in recommended_at_k if item in relevant_set])
    return hits / len(relevant)


def mean_reciprocal_rank(
    recommended: list, relevant: list, k: int | None = None
) -> float:
    """
    Calculate Mean Reciprocal Rank (MRR).
    Returns the reciprocal of the rank of the first relevant item.

    Args:
        recommended: List of recommended item IDs
        relevant: List of relevant (true positive) item IDs
        k: Optional cutoff for recommendations

    Returns:
        MRR score (float between 0 and 1)
    """
    if k is not None:
        recommended = recommended[:k]

    relevant_set = set(relevant)

    for i, item in enumerate(recommended, 1):
        if item in relevant_set:
            return 1.0 / i

    return 0.0


def ndcg_at_k(recommended: list, relevant: list, k: int) -> float:
    """
    Calculate Normalized Discounted Cumulative Gain (NDCG) at k.

    Args:
        recommended: List of recommended item IDs
        relevant: List of relevant (true positive) item IDs
        k: Number of top recommendations to consider

    Returns:
        NDCG at k (float between 0 and 1)
    """
    if k == 0 or len(relevant) == 0:
        return 0.0

    recommended_at_k = recommended[:k]
    relevant_set = set(relevant)

    # Calculate DCG
    dcg = 0.0
    for i, item in enumerate(recommended_at_k, 1):
        if item in relevant_set:
            # Binary relevance: 1 if relevant, 0 otherwise
            dcg += 1.0 / np.log2(i + 1)

    # Calculate IDCG (ideal DCG)
    idcg = 0.0
    for i in range(1, min(len(relevant), k) + 1):
        idcg += 1.0 / np.log2(i + 1)

    if idcg == 0.0:
        return 0.0

    return dcg / idcg


def hit_rate_at_k(recommended: list, relevant: list, k: int) -> float:
    """
    Calculate Hit Rate at k (binary: 1 if any relevant item in top-k, 0 otherwise).

    Args:
        recommended: List of recommended item IDs
        relevant: List of relevant (true positive) item IDs
        k: Number of top recommendations to consider

    Returns:
        1.0 if there's at least one hit, 0.0 otherwise
    """
    recommended_at_k = recommended[:k]
    relevant_set = set(relevant)

    for item in recommended_at_k:
        if item in relevant_set:
            return 1.0

    return 0.0


def coverage(all_recommendations: list[list], total_items: int) -> float:
    """
    Calculate catalog coverage: percentage of items that were recommended at least once.

    Args:
        all_recommendations: List of recommendation lists for all queries
        total_items: Total number of items in the catalog

    Returns:
        Coverage score (float between 0 and 1)
    """
    if total_items == 0:
        return 0.0

    unique_recommended = set()
    for recs in all_recommendations:
        unique_recommended.update(recs)

    return len(unique_recommended) / total_items


def diversity(recommendations: list) -> float:
    """
    Calculate diversity of recommendations (1 - repetition rate).
    Measures how diverse the recommendations are (closer to 1 is more diverse).

    Args:
        recommendations: List of recommended item IDs

    Returns:
        Diversity score (float between 0 and 1)
    """
    if len(recommendations) == 0:
        return 0.0

    unique_items = len(set(recommendations))
    return unique_items / len(recommendations)


def intra_list_diversity(recommendations: list) -> float:
    """
    Calculate intra-list diversity (diversity within a single recommendation list).
    Same as diversity() but more explicit naming.

    Args:
        recommendations: List of recommended item IDs

    Returns:
        Intra-list diversity score (float between 0 and 1)
    """
    return diversity(recommendations)
