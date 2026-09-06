"""Benchmark (hot path) de la feature `overpass`.

`create_travel_matrix` est en O(n²) sur le nombre de points (haversine sur chaque
paire) : c'est le vrai chemin chaud CPU du module. Lancé via `moon run destiny:bench`.
"""

from destiny.overpass import create_travel_matrix


def _locations(n: int = 60) -> list[dict]:
    return [{"lat": 53.0 + i * 0.01, "lng": -2.0 - i * 0.01} for i in range(n)]


def test_bench_create_travel_matrix(benchmark):
    locations = _locations()
    matrix = benchmark(create_travel_matrix, locations)
    assert len(matrix["distances"]) == 60 * 60
