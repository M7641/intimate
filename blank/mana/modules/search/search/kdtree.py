import joblib
import numpy as np
import torch
from sklearn.neighbors import KDTree


class KDTreeANN:
    """
    https://scikit-learn.org/stable/modules/generated/sklearn.neighbors.KDTree.html#sklearn.neighbors.KDTree

    It was difficult to find a ANN that worked on a Mac. On the platform we will use ScANN.
    This descision comes from the data on https://ann-benchmarks.com/index.html.

    Sci-kit Learn is used as it just works without any pain.
    """

    def __init__(
        self,
        query_model: torch.nn.Module,
    ):
        self.query_model = query_model
        self._identifiers = None
        self._searcher = None

    def index(self, candidate_embeddings: np.ndarray, identifiers: list) -> None:
        # How does one make sure the identifiers are mapped correctly?
        self._searcher = KDTree(candidate_embeddings)
        self._identifiers = identifiers

    def query(
        self,
        queries: torch.Tensor | dict[str, torch.Tensor],
        k: int = 5,
        return_distances: bool = False,
    ):
        if self._searcher is None or self._identifiers is None:
            raise ValueError("Searcher has not been built. Please call `index` first.")

        output = self.query_model(queries).detach().numpy()

        # Ensure output has correct shape for KDTree query
        if output.ndim == 1:
            output = output.reshape(1, -1)

        distances, indices = self._searcher.query(
            output,
            k=k,
        )

        identifiers_for_query = []
        for index_list in indices:
            identifiers_for_query.append(
                [self._identifiers[int(idx)] for idx in index_list]
            )

        if return_distances:
            return_objects = distances
        else:
            return_objects = identifiers_for_query

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
