import glob
import importlib
import tomllib
from pathlib import Path

import typer
from rich.console import Console

app = typer.Typer()
console = Console()

@app.command()
def something() -> None:
    console.print("Hello, World!")

def load_cwd_namespace() -> str:
    pyproject = Path.cwd() / "pyproject.toml"
    if not pyproject.exists():
        console.print("pyproject.toml not found in current directory", style="bold red")
        raise typer.Exit(code=1)

    with open(pyproject, "r") as f:
        pyproject_file = tomllib.loads(f.read())

    return pyproject_file["project"]["name"]

for file in glob.glob(
    '**/*_cli.py',
    recursive=True,
    root_dir=Path.cwd() / "src" / load_cwd_namespace()
):
    import_dir = str(load_cwd_namespace()) + "." + str(Path(file).parent) + "." + str(Path(file).stem)
    app.add_typer(importlib.import_module(import_dir).app, name=Path(file).stem)

if __name__ == "__main__":
    app()
