"""CLI Typer exposant les commandes haut-niveau du PoC.

galicia demo                              # narratif end-to-end
galicia init [--force]                    # crée le warehouse + namespaces
galicia seed-customers --n 50
galicia seed-orders 2026-05-15 --n 200
galicia evolve                            # add discount_cents to raw.orders
galicia mart                              # (re)construit mart.revenue_by_country
galicia tables
galicia snapshots raw.orders
galicia query "SELECT country, SUM(amount_cents) FROM raw.orders o JOIN raw.customers c USING (customer_id) GROUP BY 1"
"""

from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path
from typing import Annotated

import typer

from galicia import pipeline

app = typer.Typer(
    name="galicia",
    help="DuckDB + Iceberg PoC — alternative low-cost à Redshift.",
    no_args_is_help=True,
    add_completion=False,
)

WarehouseOpt = Annotated[
    Path,
    typer.Option(help="Répertoire local jouant le rôle du bucket S3."),
]


@app.command()
def demo(warehouse: WarehouseOpt = pipeline.DEFAULT_WAREHOUSE) -> None:
    """Lance la démo narrative complète (reset le warehouse en début)."""
    from galicia.demo import main as demo_main

    raise typer.Exit(demo_main(warehouse=warehouse))


@app.command()
def init(
    warehouse: WarehouseOpt = pipeline.DEFAULT_WAREHOUSE,
    force: Annotated[
        bool, typer.Option("--force", help="Wipe un warehouse existant.")
    ] = False,
) -> None:
    """Crée le warehouse et les namespaces `raw` / `mart`."""
    if force:
        pipeline.reset(warehouse)
    else:
        pipeline.ensure(warehouse)
    typer.echo(f"warehouse prêt : {warehouse}")


@app.command("seed-customers")
def cmd_seed_customers(
    n: Annotated[int, typer.Option(help="Nombre de customers à générer.")] = 50,
    seed: Annotated[int, typer.Option(help="Graine RNG (déterministe).")] = 1,
    warehouse: WarehouseOpt = pipeline.DEFAULT_WAREHOUSE,
) -> None:
    """Ajoute un lot de customers synthétiques à raw.customers."""
    catalog = pipeline.ensure(warehouse)
    snap = pipeline.seed_customers(catalog, n=n, seed_value=seed)
    typer.echo(f"snapshot {snap}  (+{n} customers)")


@app.command("seed-orders")
def cmd_seed_orders(
    day: Annotated[str, typer.Argument(help="Jour au format YYYY-MM-DD.")],
    n: Annotated[int, typer.Option(help="Nombre d'orders à générer.")] = 200,
    seed: Annotated[int, typer.Option(help="Graine RNG.")] = 11,
    with_discount: Annotated[
        bool,
        typer.Option("--with-discount", help="Inclut la colonne discount_cents."),
    ] = False,
    warehouse: WarehouseOpt = pipeline.DEFAULT_WAREHOUSE,
) -> None:
    """Ajoute un lot d'orders synthétiques pour le jour donné."""
    catalog = pipeline.ensure(warehouse)
    snap = pipeline.seed_orders(
        catalog,
        day=datetime.fromisoformat(day).replace(tzinfo=timezone.utc),
        n=n,
        seed_value=seed,
        with_discount=with_discount,
    )
    typer.echo(f"snapshot {snap}  (+{n} orders le {day})")


@app.command()
def evolve(warehouse: WarehouseOpt = pipeline.DEFAULT_WAREHOUSE) -> None:
    """Ajoute la colonne optionnelle `discount_cents` à raw.orders."""
    catalog = pipeline.ensure(warehouse)
    pipeline.evolve_orders_schema(catalog)
    typer.echo("schéma évolué : + discount_cents (int, optionnel)")


@app.command()
def mart(warehouse: WarehouseOpt = pipeline.DEFAULT_WAREHOUSE) -> None:
    """(Re)construit mart.revenue_by_country depuis raw.* (transformation dbt-style)."""
    catalog = pipeline.ensure(warehouse)
    snap = pipeline.build_mart(catalog)
    typer.echo(f"mart.revenue_by_country snapshot {snap}")


@app.command()
def tables(warehouse: WarehouseOpt = pipeline.DEFAULT_WAREHOUSE) -> None:
    """Liste les tables du warehouse."""
    catalog = pipeline.ensure(warehouse)
    for (ns,) in catalog.list_namespaces():
        for ident in catalog.list_tables(ns):
            typer.echo(f"{ident[0]}.{ident[1]}")


@app.command()
def snapshots(
    table: Annotated[str, typer.Argument(help='Table logique, ex: "raw.orders".')],
    warehouse: WarehouseOpt = pipeline.DEFAULT_WAREHOUSE,
) -> None:
    """Liste les snapshots d'une table (du plus ancien au plus récent)."""
    catalog = pipeline.ensure(warehouse)
    ice = catalog.load_table(table)
    typer.echo(f"{'snapshot_id':<24}{'timestamp_ms':<18}operation")
    for snap in ice.snapshots():
        op = snap.summary.operation if snap.summary else "?"
        typer.echo(f"{snap.snapshot_id:<24}{snap.timestamp_ms:<18}{op}")


@app.command()
def query(
    sql: Annotated[
        str,
        typer.Argument(
            help="SQL DuckDB. Les tables Iceberg sont enregistrées comme views."
        ),
    ],
    warehouse: WarehouseOpt = pipeline.DEFAULT_WAREHOUSE,
) -> None:
    """Exécute un SQL ad-hoc via DuckDB.

    Toutes les tables Iceberg sont pré-enregistrées comme views, donc
    `SELECT * FROM raw.orders` marche directement (pas besoin d'iceberg_scan).
    """
    catalog = pipeline.ensure(warehouse)
    con = pipeline.open_duckdb()
    pipeline.register_views(con, catalog)
    con.sql(sql).show()


@app.command()
def info(warehouse: WarehouseOpt = pipeline.DEFAULT_WAREHOUSE) -> None:
    """Affiche l'empreinte disque du 'bucket'."""
    if not warehouse.exists():
        typer.echo(f"(pas de warehouse à {warehouse})")
        raise typer.Exit(1)
    n_files, total = pipeline.footprint(warehouse)
    typer.echo(f"{warehouse}: {n_files} fichiers, {total / 1024:.1f} KiB")


def main() -> None:
    app()


if __name__ == "__main__":
    main()
