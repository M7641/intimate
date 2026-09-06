# Ouroboros tiers — dependency graph and module contents

Read this when deciding which tiers to generate, or when re-syncing templates after the
canonical `blank/ouroboros` changes.

## Dependency graph

```
_logging.py (NimbusLogger)             inlined logger; no external dep. Imported by
                                     images/workflows/service. Generated with core.
types.py (Artifact TypedDict)        zero deps — foundation
   └── images.py                     Images client, deploy_image, get_files_to_include,
          │                          find_parent. Uses requests + pathspec + _logging.
          │                          Everything deploys through an image, so this is core.
          ├── workflows.py           Workflows client, deploy_workflows. Calls deploy_image.
          └── service.py             Services client, build_service, deploy_service.
                                     Calls deploy_image + polls Images.describe_image.
tenant.py (Tenant)                   Instance options / sizing. Standalone; needs rich.
                                     Only the CLI's instance-options command uses it.
cli.py                               typer app. Imports whichever clients are present.
```

`core` = `_logging.py` + `types.py` + `images.py`. It is always generated:
`deploy_workflows` and `deploy_service` both call `deploy_image`, and there is no useful
deploy without it.

## What each module exposes

- **_logging.py** — `NimbusFormatter` + `NimbusLogger`, the coloured stdlib-logging wrapper
  inlined from the monorepo's `pure.logging` so the package is self-contained.
- **types.py** — `Artifact` TypedDict (`{ "path": str|Path, "ignore_files": list[str] }`),
  the artifact-upload descriptor threaded through every deploy function.
- **images.py** — `Images` (list/describe/create/version/prune image artifacts via the
  image-management API), `deploy_image(body, artifact) -> (image_id, version_id)`,
  `get_files_to_include` (gitignore-style artifact zipping), `find_parent`.
- **workflows.py** — `Workflows` (CRUD against the workflows API),
  `deploy_workflows(name, target, specs, ...)` which builds a workflow image then
  creates/updates each workflow spec, stamping the new image id onto every step.
- **service.py** — `Services` (CRUD against the webapps API), `build_service`,
  `deploy_service(...)` which builds an image, **waits for the build to succeed**
  (polls up to 600s), then creates/updates the webapp/api service.
- **tenant.py** — `Tenant.get_instance_options(entity_type)` and a rich-table
  `print_instance_options` with estimated Fargate daily cost. Pure read/no deploy.
- **cli.py** — typer commands: `list-/describe-images` (core), `list-/describe-workflows`
  (workflow), `list-/describe-services` (service), `instance-options` (tenant).

## Runtime contract (all tiers)

- `API_KEY` env var → sent as the `Authorization` header on every request.
- `TENANT` env var → read by `deploy_image` for logging context.
- All clients point at `https://service.nimbus.example/...` base URLs hardcoded on the class.

## Re-syncing templates

Templates carry a `.tmpl` suffix so the repo's `ruff --fix` pre-commit hook (globs `*.py`)
leaves them alone. The module templates keep the original `from pure.logging import
NimbusLogger` line verbatim — `bootstrap.py` rewrites it to `from ouroboros._logging import
NimbusLogger` at generation — so re-syncing stays a plain copy:

```bash
OB=../../../blank/ouroboros/src/ouroboros          # from the skill root, adjust as needed
for m in types images workflows service tenant; do cp $OB/$m.py templates/src/ouroboros/$m.py.tmpl; done
# _logging is the NimbusFormatter + NimbusLogger from pure (drop the unused LogHandler):
#   templates/src/ouroboros/_logging.py.tmpl  <-  blank/pure/src/pure/logging.py
```

`cli.py` is stored split into `templates/cli/{_header,images,workflows,services,tenant,_footer}.py.tmpl`.
If the canonical `cli.py` changes, re-split it: `_header.py.tmpl` holds the json/typer/rich
imports with a `__CLIENT_IMPORTS__` placeholder (the script fills it per tier), each command
pair goes in its tier file, and `_footer.py.tmpl` holds the `if __name__ == "__main__"` block.
