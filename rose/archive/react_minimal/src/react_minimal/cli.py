import socket
import subprocess
from typing import Annotated

import typer
from joblib import cpu_count

app = typer.Typer()


@app.command()
def run_app(
    force_multi_core: Annotated[
        bool,
        typer.Option("--force", "-f", help="Run on multiple cores"),
    ] = False,
) -> None:
    """
    Examples:
         uv run api run-app
    """
    gunicorn_args = [
        "uvicorn",
        "--host",
        "0.0.0.0", # noqa: S104
        "--port",
        "8050",
        "--workers",
        str(cpu_count()),
    ]

    if any([i in socket.gethostname() for i in ["workspace", "local"]] + [force_multi_core]):
        gunicorn_args += [
            "--reload",
            "--log-level",
            "debug",
        ]

    subprocess.run(
        [*gunicorn_args, "react_minimal.api:app"],
        check=True,
    )
