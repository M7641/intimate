#!/usr/bin/env python3
"""Regenerate the `ouroboros` Nimbus platform deploy client from bundled templates.

`ouroboros` can't always be imported (it lives in a specific monorepo), so this
script rebuilds it from scratch in any target directory. You pick how much to
generate via tiers, so a project that only needs to deploy a workflow gets just
the workflow + image code and nothing else.

The generated package is fully self-contained: the canonical source imports a
shared `pure.logging.NimbusLogger`, but that dependency is inlined here as
`ouroboros/_logging.py` and the imports are rewritten on generation, so the
result has no monorepo dependency and `uv sync`s anywhere.

Tier model (see references/tiers.md for the dependency graph):

    core      types.py + images.py + _logging.py   always included
    workflow  workflows.py                          deploy_workflows + Workflows client
    service   service.py                            deploy_service + Services client
    tenant    tenant.py                             Tenant instance-options (needs rich)
    cli       cli.py                                 typer CLI, scoped to the present tiers

Examples:
    python bootstrap.py --target ../my-pipeline --tiers workflow
    python bootstrap.py --target ../my-pipeline --tiers workflow,cli
    python bootstrap.py --target . --all
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

SKILL_ROOT = Path(__file__).resolve().parent.parent
TEMPLATES = SKILL_ROOT / "templates"
SRC = TEMPLATES / "src" / "ouroboros"
CLI = TEMPLATES / "cli"

# Templates are stored with a .tmpl suffix so the repo's ruff pre-commit hook
# (which globs *.py) leaves them untouched. read_template() appends it.
TMPL = ".tmpl"

# The canonical modules import the monorepo logger; rewrite it to the inlined one.
PURE_IMPORT = "from pure.logging import NimbusLogger"
LOCAL_IMPORT = "from ouroboros._logging import NimbusLogger"

# Each functional tier maps to the module file(s) it contributes, the CLI command
# fragment that exposes it, the client class the CLI must import, and any
# dependency it adds beyond the always-present base. "core" is implicit.
TIERS = {
    "core": {
        "modules": ["_logging.py", "types.py", "images.py"],
        "cli_fragment": "images.py",
        "client_import": "from ouroboros.images import Images",
        "deps": [],  # base deps live in BASE_DEPS
    },
    "workflow": {
        "modules": ["workflows.py"],
        "cli_fragment": "workflows.py",
        "client_import": "from ouroboros.workflows import Workflows",
        "deps": [],
    },
    "service": {
        "modules": ["service.py"],
        "cli_fragment": "services.py",
        "client_import": "from ouroboros.service import Services",
        "deps": [],
    },
    "tenant": {
        "modules": ["tenant.py"],
        "cli_fragment": "tenant.py",
        "client_import": "from ouroboros.tenant import Tenant",
        "deps": ['"rich>=13.0.0"'],
    },
}
FUNCTIONAL_TIERS = ["core", "workflow", "service", "tenant"]  # ordered; excludes cli
# Always needed: images.py uses requests + pathspec. No `pure` — logging is inlined.
BASE_DEPS = ['"requests>=2.32.5"', '"pathspec>=0.12.1"']
PYTHON_VERSION = "3.13"


def read_template(path: Path) -> str:
    """Read a template file (stored as <name>.tmpl) and rewrite the pure import."""
    return (
        path.with_suffix(path.suffix + TMPL)
        .read_text()
        .replace(PURE_IMPORT, LOCAL_IMPORT)
    )


def resolve_tiers(raw: list[str], want_all: bool) -> list[str]:
    """Normalise the requested tiers: core is mandatory, validate the rest."""
    if want_all:
        return [*FUNCTIONAL_TIERS, "cli"]
    requested = {t.strip() for arg in raw for t in arg.split(",") if t.strip()}
    valid = set(FUNCTIONAL_TIERS) | {"cli"}
    unknown = requested - valid
    if unknown:
        sys.exit(
            f"Unknown tier(s): {', '.join(sorted(unknown))}. "
            f"Valid tiers: {', '.join(sorted(valid))}."
        )
    requested.add("core")  # everything deploys through an image
    ordered = [t for t in FUNCTIONAL_TIERS if t in requested]
    if "cli" in requested:
        ordered.append("cli")
    return ordered


def compute_dependencies(tiers: list[str]) -> list[str]:
    deps = list(BASE_DEPS)
    for tier in tiers:
        if tier == "cli":
            continue
        deps.extend(TIERS[tier]["deps"])
    if "cli" in tiers:
        deps.append('"typer>=0.19.2"')
        if '"rich>=13.0.0"' not in deps:  # CLI uses rich.console too
            deps.append('"rich>=13.0.0"')
    # de-dupe, preserve order
    seen, out = set(), []
    for d in deps:
        if d not in seen:
            seen.add(d)
            out.append(d)
    return out


def assemble_cli(tiers: list[str]) -> str:
    """Build cli.py from the header + one fragment per present functional tier."""
    present = [t for t in FUNCTIONAL_TIERS if t in tiers]
    imports = "\n".join(TIERS[t]["client_import"] for t in present)
    header = read_template(CLI / "_header.py").replace("__CLIENT_IMPORTS__", imports)
    body = "".join(read_template(CLI / TIERS[t]["cli_fragment"]) for t in present)
    footer = read_template(CLI / "_footer.py")
    return header + body + footer


def render_pyproject(tiers: list[str]) -> str:
    deps = compute_dependencies(tiers)
    deps_block = "".join(f"    {d},\n" for d in deps)
    scripts = ""
    if "cli" in tiers:
        scripts = '\n[project.scripts]\nouroboros = "ouroboros.cli:app"\n'
    return f"""[project]
name = "ouroboros"
version = "0.1.0"
description = "Nimbus platform deploy client (bootstrapped)"
readme = "README.md"
requires-python = ">={PYTHON_VERSION}"
dependencies = [
{deps_block}]
{scripts}
[build-system]
requires = ["uv_build>=0.8.23,<0.9.0"]
build-backend = "uv_build"
"""


def render_moon_yml() -> str:
    return (
        "$schema: 'https://moonrepo.dev/schemas/project.json'\n"
        "layer: 'library'\n"
        "language: 'python'\n"
    )


def render_readme(tiers: list[str]) -> str:
    listed = ", ".join(tiers)
    return f"# Ouroboros\n\nBootstrapped deploy client. Generated tiers: {listed}.\n"


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)
    print(f"  wrote {path}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--target",
        required=True,
        type=Path,
        help="Directory to generate the ouroboros project into",
    )
    parser.add_argument(
        "--tiers",
        action="append",
        default=[],
        help="Comma-separated tiers (workflow,service,tenant,cli). core is always included.",
    )
    parser.add_argument(
        "--all", dest="want_all", action="store_true", help="Generate every tier"
    )
    parser.add_argument(
        "--force", action="store_true", help="Overwrite an existing non-empty target"
    )
    args = parser.parse_args()

    if not args.tiers and not args.want_all:
        sys.exit("Specify --tiers (e.g. --tiers workflow) or --all.")

    tiers = resolve_tiers(args.tiers, args.want_all)
    project = args.target.resolve()
    pkg = project / "src" / "ouroboros"

    if pkg.exists() and any(pkg.iterdir()) and not args.force:
        sys.exit(
            f"{pkg} already exists and is non-empty. Re-run with --force to overwrite."
        )

    print(f"Bootstrapping ouroboros tiers [{', '.join(tiers)}] into {project}")

    # 1. Module files for the selected functional tiers (pure import rewritten).
    write(pkg / "__init__.py", "")
    for tier in tiers:
        if tier == "cli":
            continue
        for module in TIERS[tier]["modules"]:
            write(pkg / module, read_template(SRC / module))

    # 2. CLI, assembled from the present tiers' fragments.
    if "cli" in tiers:
        write(pkg / "cli.py", assemble_cli(tiers))

    # 3. Project scaffold.
    write(project / "pyproject.toml", render_pyproject(tiers))
    write(project / "moon.yml", render_moon_yml())
    write(project / ".python-version", f"{PYTHON_VERSION}\n")
    write(project / "README.md", render_readme(tiers))

    print("\nDone. Next steps:")
    print(f"  cd {project} && uv sync")
    print("  export API_KEY=... TENANT=...   # required at runtime")
    if "cli" in tiers:
        print("  uv run ouroboros --help")


if __name__ == "__main__":
    main()
