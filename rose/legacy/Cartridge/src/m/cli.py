import itertools
import json
import os
import shutil
import subprocess
import sys
import tomllib
from collections import Counter
from pathlib import Path

import typer
from cookiecutter.main import cookiecutter
from rich.console import Console
from typing_extensions import Annotated

app = typer.Typer()
console = Console()

def load_cwd_namespace() -> str:
    pyproject = Path.cwd() / "pyproject.toml"
    if not pyproject.exists():
        console.print("pyproject.toml not found in current directory", style="bold red")
        raise typer.Exit(code=1)

    with open(pyproject, "r") as f:
        pyproject_file = tomllib.loads(f.read())

    return pyproject_file["project"]["name"]


def load_requirements(source_dir: Path) -> list:
    requirements = source_dir.read_text(encoding="utf-8").split("\n")
    requirements = [x for x in requirements if x]
    return requirements


def record_package(module: str, requirements: list) -> None:
    package_usage = Path.cwd() / "package_usage.json"

    if not package_usage.exists():
        package_usage.touch()
        package_usage.write_text("{}")

    with package_usage.open(encoding="utf-8") as f:
        package_record = json.load(f)

    package_record[module] = requirements

    with package_usage.open("w", encoding="utf-8") as f:
        json.dump(package_record, f)

def load_package_usage() -> dict:
    package_usage = Path.cwd() / "package_usage.json"

    if not package_usage.exists():
        return {}

    with package_usage.open(encoding="utf-8") as f:
        return json.load(f)

def packages_to_remove(module: str) -> list:
    """
    Convoluted, but the idea is to count the number of moudules that require a package
    take 1 away and those with a count of 0 are no longer referenced and therefore safe to remove.
    """
    package_record = load_package_usage()
    packages_potentially_safe_to_remove = package_record[module]
    all_packages = [value for value in package_record.values()]

    all_packages = [value for value in package_record.values()]
    packages = list(itertools.chain.from_iterable(all_packages))
    occurrences = Counter(packages)

    for package in occurrences:
        if package in packages_potentially_safe_to_remove:
            occurrences[package] -= 1

    packages_safe_to_remove = [i for i in filter(lambda x: occurrences[x] <= 0, occurrences)]

    return packages_safe_to_remove


def clean_package_json(module: str):
    package_record = load_package_usage()
    package_record.pop(module)

    package_usage = Path.cwd() / "package_usage.json"
    with package_usage.open("w", encoding="utf-8") as f:
        json.dump(package_record, f)


@app.command()
def install(
    module: Annotated[str, typer.Option(help="Package to install")] = "database",
):
    """
    Will keep hold of this incase we want to go more modular where each module is a package
    with it's own pyproject.toml.
    """

    console.print(f"Adding {module}...")
    currnet_dir = Path(__file__).parents[2] / "modules" / module

    os.chdir(currnet_dir)
    subprocess.run([sys.executable, "-m", "pip", "install", "-e", "."], check=True)


@app.command()
def add(
    module: Annotated[str, typer.Option(
        "--moudle", "-m",
        help="Package to add",
        )
    ] = "database",
):
    console.print(f"Adding {module}...")

    name_space = load_cwd_namespace()

    source_dir = Path(__file__).parents[2] / "modules" / module
    target_dir = Path.cwd() / "src" / name_space / module

    if target_dir.exists():
        console.print(f"{module} already exists in src/{name_space}", style="bold red")
        raise typer.Exit(code=1)

    shutil.copytree(
        source_dir,
        target_dir,
        dirs_exist_ok=False,
        ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "build", "*.egg-info", "requirements.txt"),
    )

    requirement_file = source_dir / "requirements.txt"

    if requirement_file.exists():
        convert = load_requirements(requirement_file)
        for i in convert:
            subprocess.run([sys.executable, "-m", "uv", "add", i], check=True)

        record_package(module, convert)



@app.command()
def remove(
    module: Annotated[str, typer.Option(
        "--module", "-m",
        help="Package to remove"
        )
    ] = "database",
):
    console.print(f"Removing {module}...")

    name_space = load_cwd_namespace()

    target_dir = Path.cwd() / "src" / name_space / module

    shutil.rmtree(target_dir)

    requirement_file = Path(__file__).parents[2] / "modules" / module / "requirements.txt"

    if requirement_file.exists():
        convert = packages_to_remove(module)
        for i in convert:
            subprocess.run([sys.executable, "-m", "uv", "remove", i], check=True)

    clean_package_json(module)


@app.command()
def init():
    cookiecutter_loc = Path(__file__).parent / "cookiecutter"
    cookiecutter(str(cookiecutter_loc))

if __name__ == "__main__":
    app()
