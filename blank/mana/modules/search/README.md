# search

Nearest-neighbour search over embeddings. Distributed as `mana-search`,
imported as the bare top-level `search` (crate-style — no `mana.` prefix).

`ANNFacade` picks a backend by platform: `ScANN` on Linux (the deployment
target), `KDTree` (scikit-learn) everywhere else. Both share the same
`index` / `query` / `save_index` / `load_index` surface.

```python
from search import ANNFacade

ann = ANNFacade(query_model)          # backend chosen by OS, or method="kdtree"/"scann"
ann.index(candidate_embeddings, identifiers)
neighbours = ann.query(queries, k=5)
```
