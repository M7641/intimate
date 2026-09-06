"""Le 'bucket S3' local et le catalogue Iceberg.

En prod : `warehouse` = `s3://my-bucket/`, `uri` = `glue://...` ou un Nessie
self-hosted. Ici : un répertoire et un SQLite. L'API PyIceberg est identique
dans les deux cas — c'est ça l'intérêt d'un standard de table format.
"""

from __future__ import annotations

from pathlib import Path

from pyiceberg.catalog.sql import SqlCatalog


def open_catalog(root: Path) -> SqlCatalog:
    root.mkdir(parents=True, exist_ok=True)
    return SqlCatalog(
        "galicia",
        **{
            "uri": f"sqlite:///{root / 'catalog.db'}",
            "warehouse": f"file://{root.resolve()}",
        },
    )
