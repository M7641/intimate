"""Build a Nimbus workflow spec for the extract pipeline.

Pure spec construction lives here (no network, unit-tested); the actual upload
is the CLI ``deploy`` command, which hands the spec to
``ouroboros.deploy_workflows``. Keeping them apart means the spec *shape* is
testable without a tenant or a Docker build.

The pipeline is two stages, so the workflow is two steps: ``hierarchy``
generates the free-text ``product_type`` from the warehouse, then ``condense``
canonicalises it into the concept taxonomy. ``condense`` runs after
``hierarchy`` (its ``parents``).
"""

from __future__ import annotations


def build_workflow_spec(
    *,
    name: str,
    target: str,
    department: str | None = None,
    division: str | None = None,
    sample: int = 1000,
    batch_size: int = 8,
    group_size: int = 1,
    constrained: bool = False,
    cache: bool = False,
    min_support: int = 3,
    model: str = "qwen2.5-1.5b",
    instance_type: int = 26,
    storage: str = "10GB",
    schedule: str | None = None,
    secrets: tuple[str, ...] = (),
) -> dict:
    """Return a Nimbus workflow spec for the extract pipeline.

    ``deploy_workflows`` stamps the freshly-built image id onto every step, so
    steps carry no ``imageId`` here. Warehouse auth uses the Nimbus oauth path
    (``SNOWFLAKE_AUTH_TYPE=oauth``), matching
    :class:`extract.database.SnowflakeConnector`; the Nimbus runtime
    injects ``API_KEY`` into the step environment automatically, so it is *not*
    declared as a secret. ``secrets`` is for any extra **tenant-registered**
    secret names a step needs — names not registered on the tenant are rejected.
    ``target`` tags the run and is exposed as ``TARGET_ENV``.

    A blank ``schedule`` yields no triggers (a manual-run workflow); a cron
    string yields a single cron trigger.
    """
    scope: list[str] = []
    if division:
        scope += ["--division", division]
    if department:
        scope += ["--department", department]

    env = {"SNOWFLAKE_AUTH_TYPE": "oauth", "TARGET_ENV": target}
    resources = {"instanceTypeId": instance_type, "storage": storage}
    parameters: dict = {"env": env}
    # Only declare secrets when asked: the API validates each name against the
    # tenant's registered secrets and 400s on an unknown one.
    if secrets:
        parameters["secrets"] = list(secrets)

    hierarchy_cmd = (
        f"uv run extract hierarchy --sample {sample} "
        f"--batch-size {batch_size} --model {model}"
    )
    if group_size > 1:
        hierarchy_cmd += f" --group-size {group_size}"
    if constrained:
        hierarchy_cmd += " --constrained"
    if cache:
        hierarchy_cmd += " --cache"  # warehouse-backed, so it survives the step
    if scope:
        hierarchy_cmd += " " + " ".join(scope)

    steps = {
        "hierarchy": {
            "command": hierarchy_cmd,
            "resources": resources,
            "parameters": parameters,
        },
        "condense": {
            "command": f"uv run extract condense --min-support {min_support}",
            "parents": ["hierarchy"],
            "resources": resources,
            "parameters": parameters,
        },
    }

    # Manual workflow = no triggers. A cron string becomes a single trigger.
    triggers = [{"cron": schedule}] if schedule else []
    tags = [{"name": "extract"}, {"name": target}]
    if department:
        tags.append({"name": department.lower()})

    return {
        "name": name,
        "triggers": triggers,
        "tags": tags,
        "steps": steps,
    }
