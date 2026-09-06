import shutil
import subprocess
from pathlib import Path

import typer
from rich.console import Console
from rich.table import Table

app = typer.Typer(help="nix-stack — orchestrate the Nix-reproducible dev environment")
console = Console()

PROJECT_DIR = Path(__file__).resolve().parent.parent.parent  # rose/nix_stack/
BACKEND_DIR = PROJECT_DIR / "backend"
FRONTEND_DIR = PROJECT_DIR / "frontend"


def _run(cmd: list[str], cwd: Path | None = None) -> None:
    """Run a command, streaming output to terminal."""
    result = subprocess.run(cmd, cwd=cwd)
    if result.returncode != 0:
        raise typer.Exit(result.returncode)


@app.command()
def check() -> None:
    """Verify that all required tools are available and print their versions."""
    tools = {
        "rustc": ["rustc", "--version"],
        "cargo": ["cargo", "--version"],
        "node": ["node", "--version"],
        "npm": ["npm", "--version"],
        "python3": ["python3", "--version"],
        "uv": ["uv", "--version"],
    }

    table = Table(title="Environment Check")
    table.add_column("Tool")
    table.add_column("Status")
    table.add_column("Version")

    all_ok = True
    for name, cmd in tools.items():
        if shutil.which(cmd[0]):
            result = subprocess.run(cmd, capture_output=True, text=True)
            version = (result.stdout.strip() or result.stderr.strip())
            table.add_row(name, "[green]OK[/green]", version)
        else:
            table.add_row(name, "[red]MISSING[/red]", "-")
            all_ok = False

    console.print(table)

    if not all_ok:
        console.print("\n[red]Some tools are missing. Enter the dev shell first: nix develop[/red]")
        raise typer.Exit(code=1)
    console.print("\n[green]All tools available.[/green]")


@app.command()
def dev(
    backend_only: bool = typer.Option(False, "--backend", help="Start only the Rust backend"),
    frontend_only: bool = typer.Option(False, "--frontend", help="Start only the SolidJS frontend"),
) -> None:
    """Start development servers (backend on :3000, frontend on :5173).

    The Vite dev server proxies /api/* to the Rust backend.
    """
    processes: list[subprocess.Popen] = []

    try:
        if not frontend_only:
            console.print("[blue]Starting Rust backend on http://localhost:3000 ...[/blue]")
            processes.append(subprocess.Popen(["cargo", "run"], cwd=BACKEND_DIR))

        if not backend_only:
            console.print("[blue]Starting Vite dev server on http://localhost:5173 ...[/blue]")
            processes.append(subprocess.Popen(["npm", "run", "dev"], cwd=FRONTEND_DIR))

        console.print("\n[green]Dev servers running. Ctrl+C to stop.[/green]\n")

        for p in processes:
            p.wait()

    except KeyboardInterrupt:
        console.print("\n[yellow]Shutting down...[/yellow]")
        for p in processes:
            p.terminate()
        for p in processes:
            p.wait()


@app.command()
def build() -> None:
    """Build all artifacts (Rust release binary + Vite production bundle)."""
    console.print("[blue]Building frontend...[/blue]")
    _run(["npm", "install"], cwd=FRONTEND_DIR)
    _run(["npm", "run", "build"], cwd=FRONTEND_DIR)

    console.print("[blue]Building backend (release)...[/blue]")
    _run(["cargo", "build", "--release"], cwd=BACKEND_DIR)

    console.print("[green]Build complete.[/green]")
    console.print(f"  Backend binary: {BACKEND_DIR / 'target' / 'release' / 'nix-stack-backend'}")
    console.print(f"  Frontend dist:  {FRONTEND_DIR / 'dist'}")


if __name__ == "__main__":
    app()
