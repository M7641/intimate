"""Tests unitaires de la feature `overpass`, à côté du code (style Rust).

On cible la partie pure-CPU `create_travel_matrix` (le reste du module fait du
réseau / écrit des fichiers, donc hors périmètre du test unitaire).
"""

from destiny.overpass import create_travel_matrix


def test_create_travel_matrix_empty():
    matrix = create_travel_matrix([])
    assert matrix == {"distances": [], "travelTimes": []}


def test_create_travel_matrix_single_location():
    matrix = create_travel_matrix([{"lat": 53.0, "lng": -2.0}])
    assert matrix == {"distances": [0], "travelTimes": [0]}


def test_create_travel_matrix_is_square_with_zero_diagonal_and_symmetric():
    locations = [
        {"lat": 53.4732, "lng": -2.2661},
        {"lat": 53.4794, "lng": -2.2413},
        {"lat": 53.4808, "lng": -2.2426},
    ]
    matrix = create_travel_matrix(locations)

    n = len(locations)
    assert len(matrix["distances"]) == n * n
    assert len(matrix["travelTimes"]) == n * n

    # Diagonale nulle (i == j) et matrice symétrique (d[i][j] == d[j][i]).
    for i in range(n):
        assert matrix["distances"][i * n + i] == 0
        for j in range(n):
            assert matrix["distances"][i * n + j] == matrix["distances"][j * n + i]

    # Au moins une distance hors-diagonale est strictement positive.
    assert any(d > 0 for d in matrix["distances"])
