"""Démo end-to-end narrative : ingestion → time-travel → évolution de schéma → mart.

Lance avec : `galicia demo` (CLI Typer) ou `python -m galicia.demo`.

Toute la mécanique vit dans `pipeline.py` ; ce module n'est qu'une couche de
présentation qui raconte l'histoire de chaque étape.
"""

from __future__ import annotations

import sys
from datetime import datetime, timezone
from pathlib import Path

import duckdb

from galicia import pipeline


def _section(title: str) -> None:
    print(f"\n\033[1m── {title} ──\033[0m")


def _q(con: duckdb.DuckDBPyConnection, sql: str) -> None:
    print(f"\033[2msql>\033[0m {sql.strip()}")
    con.sql(sql).show()


def main(warehouse: Path | None = None) -> int:
    warehouse = warehouse or pipeline.DEFAULT_WAREHOUSE
    catalog = pipeline.reset(warehouse)

    _section("1. Setup — un répertoire jouera le rôle de s3://galicia/")
    print(f"warehouse : {warehouse}")
    print("catalogue : SQLite (en prod : Glue ou Nessie)")

    # ───────── ingestion initiale ─────────
    _section("2. Ingestion — customers + orders du jour 1")
    pipeline.seed_customers(catalog, n=50, seed_value=1)
    snap1 = pipeline.seed_orders(
        catalog,
        day=datetime(2026, 5, 15, tzinfo=timezone.utc),
        n=200,
        seed_value=11,
    )
    print(f"snapshot 1 : {snap1}  (200 orders écrits)")

    # ───────── DuckDB lit Iceberg ─────────
    _section("3. Lecture analytique — DuckDB pointe sur les Parquet via Iceberg")
    con = pipeline.open_duckdb()
    pipeline.register_views(con, catalog)
    _q(
        con,
        """
        SELECT status, COUNT(*) n, SUM(amount_cents)/100.0 revenue_eur
        FROM raw.orders
        GROUP BY status
        ORDER BY n DESC
        """,
    )

    # ───────── deuxième batch + time-travel ─────────
    _section("4. Append du jour 2 — nouveau snapshot, ancien toujours interrogeable")
    snap2 = pipeline.seed_orders(
        catalog,
        day=datetime(2026, 5, 16, tzinfo=timezone.utc),
        n=300,
        seed_value=22,
    )
    pipeline.register_views(con, catalog)  # refresh metadata pointers
    print(f"snapshot 2 : {snap2}  (+300 orders)")

    print("\ncount au snapshot courant :")
    _q(con, "SELECT COUNT(*) AS n FROM raw.orders")

    print("\ncount au snapshot 1 (time-travel via PyIceberg → arrow → DuckDB) :")
    orders_ice = catalog.load_table("raw.orders")
    arrow_old = orders_ice.scan(snapshot_id=snap1).to_arrow()
    con.register("orders_as_of_d1", arrow_old)
    _q(con, "SELECT COUNT(*) AS n FROM orders_as_of_d1")

    # ───────── évolution de schéma ─────────
    _section("5. Évolution de schéma — ajout d'une colonne sans réécriture")
    pipeline.evolve_orders_schema(catalog)
    orders_ice = catalog.load_table("raw.orders")
    print("schéma après évolution :")
    for f in orders_ice.schema().fields:
        print(f"  - {f.name}: {f.field_type}")

    pipeline.seed_orders(
        catalog,
        day=datetime(2026, 5, 17, tzinfo=timezone.utc),
        n=150,
        seed_value=33,
        with_discount=True,
    )
    pipeline.register_views(con, catalog)
    print("\n+150 orders avec discount populé partiellement")
    _q(
        con,
        """
        SELECT discount_cents IS NOT NULL AS has_discount, COUNT(*) n
        FROM raw.orders
        GROUP BY 1
        ORDER BY 1
        """,
    )

    # ───────── transformation 'dbt-style' ─────────
    _section("6. Transformation dbt-style — DuckDB lit Iceberg, écrit Iceberg")
    print(
        "\033[2m-- même SQL qu'en Redshift, modulo iceberg_scan() qui devient un view\033[0m"
    )
    pipeline.build_mart(catalog)
    pipeline.register_views(con, catalog)

    _section("7. Lecture du mart")
    _q(con, "SELECT * FROM mart.revenue_by_country")

    # ───────── footprint ─────────
    _section("8. Empreinte disque du 'bucket'")
    n_files, total = pipeline.footprint(warehouse)
    print(f"{n_files} fichiers, {total / 1024:.1f} KiB au total dans {warehouse}")
    print("(en prod ces mêmes fichiers vivraient sur S3 à ~$0.023/GB/mois)")

    return 0


if __name__ == "__main__":
    sys.exit(main())
