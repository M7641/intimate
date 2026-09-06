"""The workflow spec builder is pure data — test its shape, no network."""

from __future__ import annotations

from extract.deploy import build_workflow_spec


def test_two_steps_with_condense_after_hierarchy():
    spec = build_workflow_spec(name="extract-furniture", target="dev")
    assert set(spec["steps"]) == {"hierarchy", "condense"}
    assert spec["steps"]["condense"]["parents"] == ["hierarchy"]
    # hierarchy is the root step — no parents.
    assert "parents" not in spec["steps"]["hierarchy"]


def test_department_flows_into_command_and_tags():
    spec = build_workflow_spec(
        name="extract-furniture", target="prod", department="FURNITURE", sample=500
    )
    cmd = spec["steps"]["hierarchy"]["command"]
    assert "--department FURNITURE" in cmd
    assert "--sample 500" in cmd
    assert {"name": "furniture"} in spec["tags"]


def test_group_c_levers_flow_into_the_hierarchy_command():
    spec = build_workflow_spec(
        name="x", target="dev", constrained=True, group_size=10, cache=True
    )
    cmd = spec["steps"]["hierarchy"]["command"]
    assert "--constrained" in cmd
    assert "--group-size 10" in cmd
    assert "--cache" in cmd


def test_group_c_levers_are_omitted_by_default():
    cmd = build_workflow_spec(name="x", target="dev")["steps"]["hierarchy"]["command"]
    assert "--constrained" not in cmd
    assert "--group-size" not in cmd
    assert "--cache" not in cmd


def test_oauth_warehouse_auth_with_no_declared_secret_by_default():
    # The Nimbus runtime injects API_KEY; declaring it as a secret 400s, so the
    # default spec carries env-only parameters and no `secrets` key.
    spec = build_workflow_spec(name="x", target="dev")
    for step in spec["steps"].values():
        assert step["parameters"]["env"]["SNOWFLAKE_AUTH_TYPE"] == "oauth"
        assert "secrets" not in step["parameters"]


def test_explicit_secrets_are_declared_on_every_step():
    spec = build_workflow_spec(name="x", target="dev", secrets=("SNOWFLAKE_PASSWORD",))
    for step in spec["steps"].values():
        assert step["parameters"]["secrets"] == ["SNOWFLAKE_PASSWORD"]


def test_schedule_becomes_cron_trigger_else_empty():
    # A manual workflow declares no triggers; the API rejects {"manual": true}.
    manual = build_workflow_spec(name="x", target="dev")
    assert manual["triggers"] == []
    cron = build_workflow_spec(name="x", target="dev", schedule="0 6 * * *")
    assert cron["triggers"] == [{"cron": "0 6 * * *"}]


def test_spec_omits_unsupported_description_key():
    # The workflow create API rejects an unspecified top-level `description`.
    spec = build_workflow_spec(name="x", target="dev")
    assert "description" not in spec


def test_steps_carry_no_image_id():
    # deploy_workflows stamps the built image id onto each step.
    spec = build_workflow_spec(name="x", target="dev")
    assert all("imageId" not in step for step in spec["steps"].values())
