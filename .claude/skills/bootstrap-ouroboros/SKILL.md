---
name: bootstrap-ouroboros
description: >-
  Regenerate the `ouroboros` Nimbus platform deploy client from scratch in any
  directory, instead of importing it from the monorepo. Generates only the tiers
  you ask for, so a project that just needs to deploy a workflow gets the Workflow
  + Image code and nothing else. Use this whenever the user wants to "bootstrap
  ouroboros", "regenerate ouroboros", "recreate the deploy client", "set up
  ouroboros in a new project/repo", "I can't import ouroboros here, make it from
  fresh", or wants just part of it — "I only need the workflow and image classes",
  "scaffold the Nimbus workflow/webapp/image deploy code", "give me deploy_workflows
  / deploy_service / the Images client standalone". Trigger even when the user
  names a capability (deploy a workflow, deploy a webapp, list images, instance
  options) rather than the word "ouroboros", as long as the Nimbus deploy client is
  what they need. Not for editing the canonical copy at blank/ouroboros itself.
---

# Bootstrap ouroboros

`ouroboros` is the Nimbus platform deploy client (`deploy_workflows`, `deploy_service`,
the `Images`/`Workflows`/`Services`/`Tenant` clients, and a typer CLI). It lives at
`blank/ouroboros` in this monorepo and can't always be imported elsewhere. This skill
**regenerates it from bundled templates** into any target directory, building **only the
tiers requested** so you don't carry code you won't use.

The generated package is **fully self-contained**: the canonical source imports the
monorepo's `pure.logging.NimbusLogger`, but the skill inlines that as `ouroboros/_logging.py`
and rewrites the imports on generation. The result has no `pure`/workspace dependency and
`uv sync`s anywhere.

## The one thing to run

A deterministic script does the assembly — prefer it over hand-copying files, because it
computes the dependency list, scopes the CLI to the present tiers, and rewrites the logging
import to the inlined module. Hand assembly gets these wrong.

```bash
python3 scripts/bootstrap.py --target <dir> --tiers <tiers> [--force]
```

Run it from the skill directory (it locates its templates relative to itself).

## Tiers — generate a little at a time

`core` (types + images) is **always** included because every deployment uploads an
image first; the other tiers are additive. Pick the smallest set that covers the goal.

| Tier       | Adds                                   | Public surface                                  | Extra deps     |
| ---------- | -------------------------------------- | ----------------------------------------------- | -------------- |
| `core`     | `types.py`, `images.py`, `_logging.py` | `Images`, `deploy_image`, `get_files_to_include`| requests, pathspec |
| `workflow` | `workflows.py`                         | `Workflows`, `deploy_workflows`                 | —              |
| `service`  | `service.py`                           | `Services`, `build_service`, `deploy_service`   | —              |
| `tenant`   | `tenant.py`                            | `Tenant.print_instance_options`                 | rich           |
| `cli`      | `cli.py`                               | typer CLI, **scoped to the tiers present**      | typer, rich    |

The CLI is assembled from fragments: it exposes the image commands (core) plus a
list/describe pair for each functional tier you included, and an `instance-options`
command if `tenant` is present. Imports and the `[project.scripts]` entry are generated
to match — so `--tiers workflow,cli` yields a CLI with only image + workflow commands,
no dangling imports.

See `references/tiers.md` for the full dependency graph and what each module contains.

## Logging is inlined (no `pure`)

The canonical modules do `from pure.logging import NimbusLogger`. The skill ships that
logger as `templates/src/ouroboros/_logging.py.tmpl` (generated into every project as part
of `core`) and rewrites the import to `from ouroboros._logging import NimbusLogger` at
generation time. So there is nothing to configure — the output never references `pure`,
`pyproject.toml` lists no workspace source, and `moon.yml` has no `dependsOn`.

## Examples

```bash
# The common case: a project that only deploys a workflow. Smallest footprint.
python3 scripts/bootstrap.py --target ../my-pipeline --tiers workflow

# Same, but with a CLI to list/describe images and workflows.
python3 scripts/bootstrap.py --target ../my-pipeline --tiers workflow,cli

# Webapp/api deploys plus instance sizing, no CLI.
python3 scripts/bootstrap.py --target ../my-service --tiers service,tenant

# Everything.
python3 scripts/bootstrap.py --target ../sandbox --all
```

The project is generated directly at `<target>` (`<target>/src/ouroboros/`, `pyproject.toml`,
`moon.yml`, …).

## After generating

1. `cd <project> && uv sync` — resolves deps (just requests/pathspec, plus rich/typer if
   the relevant tiers are present). No `pure`.
2. Runtime needs two env vars the clients read directly: `API_KEY` (sent as the
   `Authorization` header) and `TENANT`. Tell the user to export them.
3. If `cli` was included: `uv run ouroboros --help` to confirm the scoped command set.
4. Quick import smoke test, e.g. for the workflow tier:
   `uv run python -c "from ouroboros.workflows import deploy_workflows"`.

## Verifying a bootstrap

The generated code is a faithful copy of the canonical source (only the logging import is
rewritten), so the useful checks are structural: every generated `.py` should `py_compile`,
the CLI (if present) should import without `NameError`/`ImportError`, and `uv sync` should
succeed. If you changed the templates, re-run the examples above and confirm all three.

## Keeping templates faithful

Templates live under `templates/` with a **`.tmpl` suffix** (e.g. `images.py.tmpl`) so the
repo's `ruff --fix` pre-commit hook — which globs `*.py` — leaves them alone; ruff would
otherwise strip "unused" imports from the CLI fragments and break assembly. The script reads
them by appending `.tmpl`.

The module templates are verbatim extracts of the canonical files **including** the original
`from pure.logging import NimbusLogger` line — the script rewrites it on generation, which keeps
re-syncing a plain copy. If `blank/ouroboros` changes meaningfully:

```bash
OB=../../../blank/ouroboros/src/ouroboros
for m in types images workflows service tenant; do cp $OB/$m.py templates/src/ouroboros/$m.py.tmpl; done
```

`_logging.py.tmpl` is sourced from `blank/pure/src/pure/logging.py` (NimbusFormatter + NimbusLogger).
`cli.py` is stored split into `templates/cli/{_header,images,workflows,services,tenant,_footer}.py.tmpl`;
if the canonical `cli.py` changes, re-split it (`_header` keeps the `__CLIENT_IMPORTS__`
placeholder the script fills per tier). This skill does not edit the canonical copy.
