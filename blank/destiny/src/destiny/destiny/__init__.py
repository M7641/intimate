"""Feature `destiny` — OSRM routing + Haversine distances.

Le `__init__` réexporte l'API publique pour que le dossier-feature se comporte
comme un simple module (`from destiny.destiny import Destiny`, et via le package
racine `from destiny import Destiny`).
"""

from .destiny import Destiny

__all__ = ["Destiny"]
