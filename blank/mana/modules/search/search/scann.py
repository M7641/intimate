import uuid

import joblib
import numpy as np
import torch
from pure.logging import NimbusLogger

logger = NimbusLogger(name=__name__).logger

try:
    from scann import scann_ops
except ImportError:
    logger.warning("ScaNN is not installed. Some functionality may be unavailable.")
    scann_ops = None


class ScANN:
    def __init__(
        self,
        query_model=None,
    ):
        self.query_model = query_model
        self._identifiers = None
        self._searcher = None

    def index(self, candidate_embeddings: np.ndarray, identifiers: list):
        self._identifiers = identifiers
        builder = scann_ops.builder(
            db=candidate_embeddings, num_neighbors=10, distance_measure="dot_product"
        )

        builder = builder.tree(
            num_leaves=100,
            num_leaves_to_search=10,
            training_iterations=12,
        )

        builder = builder.score_ah(dimensions_per_block=2)

        self._searcher = builder.build(
            shared_name=f"{uuid.uuid4()}"
        ).serialize_to_module()

    def query(
        self,
        queries: torch.Tensor | dict[str, torch.Tensor],
        k: int = 5,
        return_distances: bool = False,
    ):
        """
        Go through and get all the right data types and then totally align with
        KDTree version.
        """

        if self._searcher is None or self._identifiers is None:
            raise ValueError(
                "The `index` method must be called first to create the retrieval index."
            )

        searcher = scann_ops.searcher_from_module(self._searcher)

        if self.query_model is not None:
            queries = self.query_model(queries)

        result = searcher.search(queries, final_num_neighbors=k)
        indices = result.index
        distances = result.distance

        if len(indices) == 1:
            indices = indices[0]
            distances = distances[0]

        return_ids = []
        for i in indices:
            return_ids.append(self._identifiers[i])

        if return_distances:
            return_objects = distances
        else:
            return_objects = return_ids

        return return_objects

    def save_index(self, path: str) -> None:
        joblib.dump(
            {
                "identifiers": self._identifiers,
                "searcher": self._searcher,
            },
            path,
        )

    def load_index(self, path: str) -> None:
        data = joblib.load(path)
        self._identifiers = data["identifiers"]
        self._searcher = data["searcher"]
