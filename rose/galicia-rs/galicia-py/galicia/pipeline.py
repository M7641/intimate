"""Opérations haut-niveau sur le warehouse Iceberg.

Fonctions pures : pas de print, pas de couleur, pas de side-effect d'affichage.
La démo et le CLI sont des wrappers présentationnels par-dessus.
"""

from __future__ import annotations

import shutil
from datetime import datetime
from pathlib import Path

import duckdb
import pyarrow as pa
from pyiceberg.catalog.sql import SqlCatalog
from pyiceberg.exceptions import NamespaceAlreadyExistsError, NoSuchTableError
from pyiceberg.types import IntegerType

from galicia import seed
from galicia.warehouse import open_catalog

DEFAULT_WAREHOUSE = Path(__file__).resolve().parent.parent / "warehouse"
NAMESPACES = ("raw", "mart")


def ensure(warehouse: Path) -> SqlCatalog:
    catalog = open_catalog(warehouse)
    for ns in NAMESPACES:
        try:
            catalog.create_namespace(ns)
        except NamespaceAlreadyExistsError:
            pass
    return catalog


def reset(warehouse: Path) -> SqlCatalog:
    if warehouse.exists():
        shutil.rmtree(warehouse)
    return ensure(warehouse)


def seed_customers(catalog: SqlCatalog, *, n: int, seed_value: int = 1) -> int:
    tbl = seed.customers(n, seed=seed_value)
    try:
        ice = catalog.load_table("raw.customers")
    except NoSuchTableError:
        ice = catalog.create_table("raw.customers", schema=tbl.schema)
    ice.append(tbl)
    return ice.current_snapshot().snapshot_id


def seed_orders(
    catalog: SqlCatalog,
    *,
    day: datetime,
    n: int,
    seed_value: int = 11,
    with_discount: bool = False,
) -> int:
    customers = catalog.load_table("raw.customers").scan().to_arrow()
    cids = customers.column("customer_id").to_pylist()
    if not cids:
        raise RuntimeError("raw.customers est vide — seed-customers d'abord.")

    batch = seed.orders(n, customer_ids=cids, day=day, seed=seed_value)
    if with_discount:
        discounts = pa.array(
            [
                (amt // 10) if i % 3 == 0 else None
                for i, amt in enumerate(batch.column("amount_cents").to_pylist())
            ],
            type=pa.int32(),
        )
        batch = batch.append_column("discount_cents", discounts)

    try:
        ice = catalog.load_table("raw.orders")
    except NoSuchTableError:
        ice = catalog.create_table("raw.orders", schema=batch.schema)
    ice.append(batch)
    return ice.current_snapshot().snapshot_id


def evolve_orders_schema(catalog: SqlCatalog) -> None:
    ice = catalog.load_table("raw.orders")
    with ice.update_schema() as upd:
        upd.add_column("discount_cents", IntegerType(), required=False)
    ice.refresh()


def build_mart(catalog: SqlCatalog) -> int:
    orders = catalog.load_table("raw.orders")
    customers = catalog.load_table("raw.customers")
    con = open_duckdb()
    sql = f"""
        SELECT
          c.country,
          COUNT(*) FILTER (WHERE o.status = 'paid')                          AS paid_orders,
          SUM(o.amount_cents) FILTER (WHERE o.status = 'paid') / 100.0       AS gross_revenue_eur,
          COALESCE(SUM(o.discount_cents), 0) / 100.0                         AS total_discount_eur
        FROM iceberg_scan('{orders.metadata_location}') o
        JOIN iceberg_scan('{customers.metadata_location}') c USING (customer_id)
        GROUP BY c.country
        ORDER BY gross_revenue_eur DESC
    """
    mart_arrow = con.execute(sql).fetch_arrow_table()
    try:
        ice = catalog.load_table("mart.revenue_by_country")
        ice.overwrite(mart_arrow)
    except NoSuchTableError:
        ice = catalog.create_table("mart.revenue_by_country", schema=mart_arrow.schema)
        ice.append(mart_arrow)
    return ice.current_snapshot().snapshot_id


def open_duckdb() -> duckdb.DuckDBPyConnection:
    con = duckdb.connect()
    con.execute("INSTALL iceberg; LOAD iceberg;")
    return con


def register_views(con: duckdb.DuckDBPyConnection, catalog: SqlCatalog) -> list[str]:
    """Expose chaque table Iceberg comme une view DuckDB pour qu'on puisse
    écrire `SELECT * FROM raw.orders` au lieu de `iceberg_scan('…/metadata.json')`.

    Même rôle que `{{ ref('orders') }}` dans dbt : le SQL utilisateur ignore
    la résolution catalogue→fichiers.
    """
    registered: list[str] = []
    for (ns,) in catalog.list_namespaces():
        con.execute(f'CREATE SCHEMA IF NOT EXISTS "{ns}"')
        for ident in catalog.list_tables(ns):
            ns_name, tbl_name = ident
            tbl = catalog.load_table(ident)
            con.execute(
                f'CREATE OR REPLACE VIEW "{ns_name}"."{tbl_name}" AS '
                f"SELECT * FROM iceberg_scan('{tbl.metadata_location}')"
            )
            registered.append(f"{ns_name}.{tbl_name}")
    return registered


def footprint(warehouse: Path) -> tuple[int, int]:
    files = [p for p in warehouse.rglob("*") if p.is_file()]
    return len(files), sum(p.stat().st_size for p in files)
