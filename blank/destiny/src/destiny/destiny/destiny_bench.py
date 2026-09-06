"""Benchmarks (hot path) de la feature `destiny`.

Mesurés avec `pytest-benchmark`, lancés via `moon run destiny:bench` (jamais par
`moon run destiny:test` : ces fichiers `*_bench.py` ne sont pas collectés par
défaut). Entrées déterministes, aucun réseau — c'est du CPU pur.
"""

from destiny.destiny import Destiny


def _big_osrm_response(n_steps: int = 50, coords_per_step: int = 100) -> dict:
    """Réponse OSRM synthétique : `n_steps` étapes de `coords_per_step` points."""
    steps = []
    for s in range(n_steps):
        base = s * coords_per_step
        coords = [
            [-2.0 + (base + i) * 1e-5, 53.0 + (base + i) * 1e-5]
            for i in range(coords_per_step)
        ]
        steps.append({"geometry": {"coordinates": coords}})
    return {"routes": [{"legs": [{"steps": steps}]}]}


def _coord_pairs(n: int = 2000) -> tuple[list[tuple], list[tuple]]:
    starts = [(53.0 + i * 1e-4, -2.0 - i * 1e-4) for i in range(n)]
    ends = [(53.5 + i * 1e-4, -2.5 - i * 1e-4) for i in range(n)]
    return starts, ends


def test_bench_extract_route_nodes(benchmark):
    destiny = Destiny()
    response = _big_osrm_response()
    nodes = benchmark(destiny.extract_route_nodes, response)
    assert len(nodes) == 50 * 100


def test_bench_compute_haversine_distance(benchmark):
    destiny = Destiny()
    starts, ends = _coord_pairs()
    distances = benchmark(destiny.compute_haversine_distance, starts, ends)
    assert len(distances) == 2000
