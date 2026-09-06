"""A2 embed → cluster → categorical — see :mod:`extract.cluster.cluster`."""

from extract.cluster.cluster import (
    assign_to_centroids,
    cluster_frame,
    distinctive_terms,
    exemplars,
    greedy_clusters,
    name_clusters,
)

__all__ = [
    "assign_to_centroids",
    "cluster_frame",
    "distinctive_terms",
    "exemplars",
    "greedy_clusters",
    "name_clusters",
]
