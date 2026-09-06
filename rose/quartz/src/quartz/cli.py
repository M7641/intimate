import platform
import subprocess
from pathlib import Path
from typing import Annotated

import typer
import uvicorn
from apscheduler.schedulers.background import BackgroundScheduler
from joblib import cpu_count
from pure.logging import NimbusLogger

app = typer.Typer()
logger = NimbusLogger(name=__name__).logger

FRONTEND_DIR = Path(__file__).parents[2] / "frontend"


def background_job():
    print("Running background job...")


@app.command("build")
def build() -> None:
    """
    Build the frontend assets.

    This command does not work if the packages have not been installed.
    This can be resolved by running the 'install' command first.

    Examples:
        uv run quartz build
    """
    (FRONTEND_DIR / "dist").mkdir(exist_ok=True)

    subprocess.run(
        ["bun", "install", "--frozen-lockfile"],
        check=True,
        cwd=FRONTEND_DIR,
    )
    subprocess.run(
        ["bun", "run", "build"],
        check=True,
        cwd=FRONTEND_DIR,
    )


@app.command("install")
def install() -> None:
    """
    Install frontend dependencies.

    Examples:
        uv run quartz install
    """
    subprocess.run(
        ["bun", "install"],
        check=True,
        cwd=FRONTEND_DIR,
    )


@app.command("dev")
def dev() -> None:
    """
    Run the development server with hot reloading.

    Examples:
        uv run quartz dev
    """
    subprocess.run(
        ["bun", "run", "dev"],
        check=True,
        cwd=FRONTEND_DIR,
    )


@app.command("start")
def start(
    prod: Annotated[
        bool,
        typer.Option("--prod", "-p", help="Run in production mode"),
    ] = False,
) -> None:
    """
    Examples:
        uv run quartz start
    """
    if platform.system().lower() == "darwin":
        use_reloader = True
        log_level = "debug"
    else:
        build()
        use_reloader = False
        log_level = "info"

    # Start background scheduler in main process
    scheduler = BackgroundScheduler()
    scheduler.add_job(
        background_job,
        "cron",
        minute="*/5",
    )
    scheduler.start()

    # Run uvicorn in main process (scheduler runs in background thread)
    try:
        uvicorn.run(
            "quartz.api.api:app",
            host="0.0.0.0",
            port=8050,
            log_level=log_level,
            reload=use_reloader,
            loop="uvloop",
            http="httptools",
            workers=1 if use_reloader else cpu_count() - 1,
        )
    finally:
        scheduler.shutdown()


if __name__ == "__main__":
    app()
