import random
import subprocess
import sys
import time
from pathlib import Path
from urllib.request import Request, urlopen
from urllib.error import URLError

import typer

app = typer.Typer(help="Spawner — observability demo API")

# apocrypha/ is three levels up from src/spawner/cli.py
APOCRYPHA_DIR = Path(__file__).resolve().parent.parent.parent.parent
COMPOSE_FILE = APOCRYPHA_DIR / "docker-compose.yaml"


def _compose(*args: str) -> None:
    """Run podman compose with the apocrypha compose file.

    Output streams directly to the terminal so you see pull/build progress.
    Gives a clear error if the podman machine isn't running.
    """
    result = subprocess.run(
        ["podman", "compose", "-f", str(COMPOSE_FILE), *args],
    )
    if result.returncode != 0:
        probe = subprocess.run(
            ["podman", "machine", "info"],
            capture_output=True,
            text=True,
        )
        if "Running" not in probe.stdout:
            typer.echo(
                "Error: Podman machine is not running. Run `podman machine start` first.",
                err=True,
            )
        raise typer.Exit(1)


@app.command()
def run(
    host: str = typer.Option("0.0.0.0", help="Bind address"),
    port: int = typer.Option(8000, help="Port"),
    reload: bool = typer.Option(False, help="Enable auto-reload for development"),
) -> None:
    """Run the Spawner FastAPI app (without the backend stack)."""
    cmd = [
        sys.executable,
        "-m",
        "uvicorn",
        "spawner.api:app",
        "--host",
        host,
        "--port",
        str(port),
    ]
    if reload:
        cmd.append("--reload")
    subprocess.run(cmd, check=True)


@app.command()
def stack(
    host: str = typer.Option("0.0.0.0", help="Bind address"),
    port: int = typer.Option(8000, help="Port"),
    reload: bool = typer.Option(False, help="Enable auto-reload for development"),
) -> None:
    """Start the full observability stack (docker compose) then run Spawner."""
    typer.echo("Starting observability stack...")
    _compose("up", "-d")

    typer.echo()
    typer.echo("Stack is up:")
    typer.echo("  Grafana        http://localhost:3001")
    typer.echo("  Tempo          http://localhost:3200")
    typer.echo("  Loki           http://localhost:3100")
    typer.echo("  Prometheus     http://localhost:8050")
    typer.echo("  Pyroscope      http://localhost:4040")
    typer.echo("  OTel Collector localhost:4317 (gRPC) / localhost:4318 (HTTP)")
    typer.echo()
    typer.echo(f"Starting Spawner on http://localhost:{port} ...")
    typer.echo("  Ctrl+C stops Spawner (backends keep running).")
    typer.echo("  Run `uv run spawner down` to tear down backends.")
    typer.echo()

    cmd = [
        sys.executable,
        "-m",
        "uvicorn",
        "spawner.api:app",
        "--host",
        host,
        "--port",
        str(port),
    ]
    if reload:
        cmd.append("--reload")
    subprocess.run(cmd, check=False)


@app.command()
def down() -> None:
    """Stop the observability stack (docker compose down)."""
    _compose("down")
    typer.echo("Stack stopped.")


@app.command()
def status() -> None:
    """Show running status of the observability stack."""
    _compose("ps")


def _hit(base: str, method: str, path: str) -> int:
    """Fire a request and return the status code. Swallows errors."""
    try:
        req = Request(f"{base}{path}", method=method)
        with urlopen(req, timeout=10) as resp:
            resp.read()
            return resp.status
    except URLError:
        return 0


@app.command()
def flood(
    base: str = typer.Option("http://localhost:8000", help="Spawner base URL"),
    duration: int = typer.Option(60, help="How long to generate traffic (seconds)"),
    rps: float = typer.Option(5.0, help="Approximate requests per second"),
) -> None:
    """Generate continuous traffic across all endpoint groups.

    Run this in a second terminal while `spawner stack` is running.
    Produces a realistic mix of traces, metrics, logs, and workflow activity
    so Grafana dashboards have data to display.
    """
    delay = 1.0 / rps
    end = time.time() + duration
    sent = 0
    errors = 0
    workflows: list[str] = []

    typer.echo(f"Flooding {base} for {duration}s at ~{rps} req/s ...")
    typer.echo("Ctrl+C to stop early.\n")

    try:
        while time.time() < end:
            # Pick a random action
            action = random.choices(
                ["trace", "metric", "log", "workflow", "profile"],
                weights=[30, 25, 20, 20, 5],
            )[0]

            status = 0
            label = ""

            if action == "trace":
                path = random.choice(
                    ["/traces/simple", "/traces/nested", "/traces/error"]
                )
                status = _hit(base, "GET", path)
                label = f"GET {path}"

            elif action == "metric":
                status = _hit(base, "POST", "/metrics/order")
                label = "POST /metrics/order"

            elif action == "log":
                path = random.choice(["/logs/levels", "/logs/structured"])
                status = _hit(base, "GET", path)
                label = f"GET {path}"

            elif action == "workflow":
                # Start new workflow or advance an existing one
                if not workflows or random.random() < 0.3:
                    wf_type = random.choice(["etl", "data_pipeline", "report", "sync"])
                    try:
                        import json

                        req = Request(
                            f"{base}/workflows/?workflow_type={wf_type}", method="POST"
                        )
                        with urlopen(req, timeout=10) as resp:
                            wf_id = json.loads(resp.read())["id"]
                            workflows.append(wf_id)
                            status = resp.status
                            label = f"POST /workflows/ ({wf_id})"
                    except (URLError, KeyError):
                        status = 0
                        label = "POST /workflows/ (failed)"
                else:
                    wf_id = random.choice(workflows)
                    status = _hit(base, "POST", f"/workflows/{wf_id}/step")
                    label = f"POST /workflows/{wf_id}/step"
                    # Remove completed/failed workflows
                    if status == 400:
                        workflows.remove(wf_id)

            elif action == "profile":
                n = random.randint(15, 25)
                status = _hit(base, "GET", f"/profile/fibonacci/{n}")
                label = f"GET /profile/fibonacci/{n}"

            sent += 1
            if status == 0:
                errors += 1

            if sent % 10 == 0:
                elapsed = duration - (end - time.time())
                typer.echo(
                    f"  [{elapsed:.0f}s] {sent} requests sent, {errors} errors, {len(workflows)} active workflows (last: {label})"
                )

            time.sleep(delay)

    except KeyboardInterrupt:
        typer.echo()

    typer.echo(f"\nDone: {sent} requests in {duration}s ({errors} errors)")
    typer.echo("Open Grafana at http://localhost:3001 to see the data.")


if __name__ == "__main__":
    app()
