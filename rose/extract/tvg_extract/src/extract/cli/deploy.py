"""Deploy the extract pipeline as a Nimbus workflow."""

from __future__ import annotations

import json
import os
from pathlib import Path

import typer

from extract.cli.common import (
    BATCH_HELP,
    CONSTRAINED_HELP,
    DEPARTMENT_HELP,
    DIVISION_HELP,
    GROUP_HELP,
    MODEL_HELP,
    WAREHOUSE_CACHE_HELP,
    console,
)


def deploy(
    name: str = typer.Option("extract-furniture", help="workflow name on the tenant"),
    target: str = typer.Option("dev", help="target env, also used as a tag (dev/prod)"),
    department: str = typer.Option("FURNITURE", help=DEPARTMENT_HELP),
    division: str = typer.Option("", help=DIVISION_HELP),
    sample: int = typer.Option(1000, help="products to sample per scheduled run"),
    batch_size: int = typer.Option(8, help=BATCH_HELP),
    group_size: int = typer.Option(1, help=GROUP_HELP),
    constrained: bool = typer.Option(False, help=CONSTRAINED_HELP),
    cache: bool = typer.Option(False, help=WAREHOUSE_CACHE_HELP),
    min_support: int = typer.Option(3, help="condense promotion gate"),
    model: str = typer.Option("qwen2.5-1.5b", help=MODEL_HELP),
    schedule: str = typer.Option(
        "", help="cron schedule (e.g. '0 6 * * *'); blank = manual trigger"
    ),
    secret: list[str] = typer.Option(
        [],
        help="tenant-registered secret name to expose to each step (repeatable); "
        "API_KEY is injected by the runtime and must NOT be listed",
    ),
    dockerfile: Path = typer.Option(
        Path("Dockerfile.workflow"),
        help="workflow image Dockerfile (build context = the project root, "
        "where pyproject + uv.lock + src live)",
    ),
    dry_run: bool = typer.Option(
        False, help="print the workflow spec and exit without building/deploying"
    ),
) -> None:
    """
    Deploy the extract pipeline as a Nimbus workflow (hierarchy -> condense).

    Builds a workflow image from --dockerfile (context = project root) and registers
    a two-step workflow via the vendored deploy client
    (extract.deploy). Needs `API_KEY` and `TENANT` exported — the
    client reads them directly. Use --dry-run to inspect the spec first.

    Example:
        $ uv run extract deploy --department FURNITURE --target dev --dry-run
        $ uv run extract deploy --department FURNITURE --target prod --schedule '0 6 * * *'
    """
    from extract.deploy import build_workflow_spec

    spec = build_workflow_spec(
        name=name,
        target=target,
        department=department or None,
        division=division or None,
        sample=sample,
        batch_size=batch_size,
        group_size=group_size,
        constrained=constrained,
        cache=cache,
        min_support=min_support,
        model=model,
        schedule=schedule or None,
        secrets=tuple(secret),
    )
    if dry_run:
        console.print_json(json.dumps(spec, ensure_ascii=False))
        return

    if not os.getenv("API_KEY"):
        raise typer.BadParameter(
            "API_KEY must be set — ouroboros sends it as the Authorization header"
        )

    # Imported lazily, like the Snowflake deps: the deploy client is only needed
    # for this one command.
    from extract.deploy import deploy_workflows

    console.print(
        f"[dim]building image from {dockerfile} and deploying workflow "
        f"'{name}' (target={target})…[/dim]"
    )
    deploy_workflows(
        workflow_name=name,
        target=target,
        workflow_specs=spec,
        dockerfile_path=str(dockerfile),
        # Build context is the project root (where this command runs); the
        # client's default artifact (path=".", ignore_files=[".dockerignore"])
        # is exactly right now that both live there.
        # Bake the local API_KEY into the image so the warehouse oauth path can
        # authenticate to the Nimbus connections API at runtime (Dockerfile ARG).
        build_args=[{"name": "API_KEY", "value": os.environ["API_KEY"]}],
    )
    console.print(f"[green]Deployed[/green] workflow '{name}' (target={target})")
