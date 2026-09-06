from pathlib import Path
from typing import Annotated

import typer

from common_py.ask_warehouse.mod import GOLDEN_SET_PATH
from common_py.ask_warehouse.validation import run as run_validation

ask_app = typer.Typer(help="ask_warehouse pilot (NL → SQL).")


@ask_app.command("validate")
def validate(
    url: Annotated[
        str,
        typer.Option(
            "--url",
            "-u",
            help="Base URL of the running Welcome instance (without /api/ask).",
        ),
    ] = "http://localhost:8000",
    golden: Annotated[
        Path,
        typer.Option(
            "--golden",
            "-g",
            help="Path to the golden-set YAML.",
            exists=True,
            dir_okay=False,
        ),
    ] = GOLDEN_SET_PATH,
    email: Annotated[
        str,
        typer.Option(
            "--email",
            "-e",
            help="X-Auth-Email header value; also written to the audit log.",
        ),
    ] = "eval@northwind.local",
    threshold: Annotated[
        float,
        typer.Option(
            "--threshold",
            "-t",
            help="Pass-rate threshold; the command exits non-zero below this.",
            min=0.0,
            max=1.0,
        ),
    ] = 0.8,
) -> None:
    """Run the ask_warehouse behavioural-quality eval against a live endpoint.

    Examples:
        uv run common ask validate
        uv run common ask validate --url http://localhost:8050
        uv run common ask validate --golden /tmp/custom.yaml --threshold 0.9
    """
    code = run_validation(
        url=url,
        golden_path=golden,
        email=email,
        pass_threshold=threshold,
    )
    raise typer.Exit(code)
