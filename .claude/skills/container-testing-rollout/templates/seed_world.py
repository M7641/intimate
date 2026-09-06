"""World building for the sandbox — the one app-specific piece.

The shared runner (``common_py.testing.sandbox``) owns the container, the safety
guard, the app boot and the teardown. It asks the app for exactly one thing: a
``seed(db, env)`` coroutine that populates a *fresh, empty* Postgres with a
realistic-enough world. That coroutine is what this template is.

The same ``seed`` feeds both sandboxes — ``run_sandbox`` (host uvicorn, an
explorable world) and ``run_containerised_sandbox`` (the capacity axis, under
cgroup limits). Write it once; the CLI at the bottom wires it to both.

Copy to ``modules/<app>/sandbox/seed_world.py`` (or wherever the app keeps its
dev tooling) and edit every ``EDIT`` marker.
"""

from __future__ import annotations

import typer

from common_py.env import EnvManager
from common_py.io.db import AsyncDBActions
from common_py.testing.sandbox import run_containerised_sandbox, run_sandbox

# EDIT: import the SAME seeders / row generators your e2e suite uses. Reusing
# them (rather than hand-rolling INSERTs here) is the whole point — the sandbox
# and the tests then seed identical shapes and can never silently disagree.
# from modules.<app>.tests.seeders import WidgetSeeder, OrderSeeder, make_widget


async def seed(db: AsyncDBActions, env: EnvManager) -> None:
    """Populate a fresh container with the minimum realistic world.

    Called once, against an open pool on the *empty* seeded container, before
    the app boots. Four rules keep the world honest and the sandbox useful:

    1. **Populate what the target endpoints READ.** Create the source
       (``upstream``) tables the pages query and insert a handful of realistic
       rows — enough to render, not a full warehouse.
    2. **Create join targets empty, don't skip them.** A table a query LEFT
       JOINs to must EXIST, or the endpoint 500s on a missing relation. Leave it
       created-but-empty so the join resolves to zero rows instead of erroring.
    3. **Grant the dev identity what the menus need.** The sandbox authenticates
       every request as the dev user; grant it the permissions the app's
       navigation checks, or half the UI renders empty.
    4. **Seed into ``env.schema``.** The seeders create the schema lowercase;
       the app folds its unquoted schema name to lowercase too, so reads line up
       with no in-process patching. (Write-heavy stress needs an uppercase seed —
       see references/capacity-sizing.md.)
    """
    schema = env.schema  # lowercase; matches what the app reads

    # Reuse the e2e seeders. Each ensures its schema+table, then adds rows built
    # by the same generators the tests use. EDIT: your tables and row counts.
    #
    # await WidgetSeeder(db, schema).ensure_schema()
    # await WidgetSeeder(db, schema).add([make_widget() for _ in range(20)])
    #
    # Join target the widget query LEFT JOINs to — created, deliberately empty:
    # await OrderSeeder(db, schema).ensure_schema()

    # Grant the dev identity the menu permissions (EDIT to your permission model):
    # await db.execute_query(GRANT_DEV_MENUS_SQL.format(schema=schema))

    raise NotImplementedError("EDIT: build your world here, then delete this line")


# ── CLI: one command per sandbox, both fed the same seed ──────────────────────
#
# Wire these into the app's existing CLI (Typer here — match the house style).
# `sandbox` is the explorable world; `sandbox-container` is the capacity axis.

app = typer.Typer(help="Disposable seeded sandboxes for <app>.")

APP_IMPORT = "modules.<app>.backend.app:app"  # EDIT: uvicorn import string
FRONTEND_DIR = "modules/<app>/frontend"  # EDIT: dir holding the Vite project
APP_NAME = "<app>"  # EDIT: module name, used for image tag + container names


@app.command()
def sandbox(
    port: int = typer.Option(8060, help="Host port to serve on."),
    build: bool = typer.Option(True, help="Rebuild the frontend dist first."),
) -> None:
    """Boot an explorable seeded world (host uvicorn) and block until Ctrl-C."""
    from pathlib import Path

    run_sandbox(
        app=APP_IMPORT,
        frontend_dir=Path(FRONTEND_DIR),
        seed=seed,
        port=port,
        build=build,
    )


@app.command("sandbox-container")
def sandbox_container(
    cpus: float = typer.Option(None, help="Fractional cores, e.g. 1.5."),
    memory: str = typer.Option(None, help="Docker-style size, e.g. 512m."),
    port: int = typer.Option(8070, help="Host port to publish."),
    rebuild_image: bool = typer.Option(False, help="Force a prod-image rebuild."),
) -> None:
    """Run the prod image under cgroup limits for the capacity axis (see k6)."""
    run_containerised_sandbox(
        app_name=APP_NAME,
        seed=seed,
        cpus=cpus,
        memory=memory,
        port=port,
        rebuild_image=rebuild_image,
    )


if __name__ == "__main__":
    app()
