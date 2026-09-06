import contextlib
import glob
import logging
import os
import shutil

from nimbus.press import apps, blocks

block_client = blocks.get_client()
app_client = apps.get_client()


def build_workflow_spec() -> dict:
    """
    Build workflow spec.

    block per model?

    Parameters
    ----------
    deploy_prod: str
        Whether to deploy to production or not.
    model_names: list
        List of model names.

    Returns
    -------
    dict
        Workflow spec.
    """

    workflow_spec = {
        "body": {
            "version": "1",
            "kind": "workflow",
            "metadata": {
                "title": "Model Explainability Workflow Press",
                "name": "build-metis-workflow",
                "summary": "Webapp to explain model predictions",
                "description": "Model Explainability Workflow",
                "descriptionContentType": "text/markdown",
                "imageUrl": "https://upload.wikimedia.org/wikipedia/commons/thumb/5/53/Winged_goddess_Louvre_F32.jpg/440px-Winged_goddess_Louvre_F32.jpg",
            },
            "release": {"version": "0.0.1"},
            "config": {
                "triggers": [],
                "images": {
                    "metis-workflow-image": {
                        "context": ".",
                        "dockerfile": "Dockerfile.workflow",
                        "useCache": False,
                        "version": "0.0.1",
                        "secrets": ["GITHUB_TOKEN", "API_KEY"],
                        "buildArguments": {
                            "CONFIG_REPOSITORY": "@param:config_repository",
                            "CONFIG_FILE_NAME": "@param:config_file_name",
                        },
                    }
                },
                "steps": {},
            },
        },
        "parameters": {
            "build": [
                {
                    "name": "config_repository",
                    "required": True,
                    "type": "string",
                    "defaultValue": "nimbus-labs/ds-financeco-audiences",
                },
                {
                    "name": "config_file_name",
                    "required": True,
                    "type": "string",
                    "defaultValue": "webapps/model_explain/config.yaml",
                },
            ],
        },
        "artifact": {"path": "."},
    }

    workflow_spec["body"]["config"]["steps"] = {
        "pip-list": {
            "imageRef": "metis-workflow-image",
            "resources": {"instanceTypeId": 20, "storage": "1GB"},
            "command": "pip list",
        },
        "run-model-explain-models": {
            "imageRef": "metis-workflow-image",
            "resources": {"instanceTypeId": 23, "storage": "10GB"},
            "command": "metis build-workflow",
            "parents": ["pip-list"],
        },
    }

    return workflow_spec


def increment_version(version: str, semantic_level: str) -> str:
    """
    Increment the version number.

    Args:
        version (str): The current version number.
        semantic_level (str): The semantic level to increment (e.g., 'patch', 'minor', 'major').

    Returns:
        str: The incremented version number.
    """
    version_split = version.split(".")
    if semantic_level == "patch":
        version_increment = int(version_split[2]) + 1
        new_version = f"{version_split[0]}.{version_split[1]}.{version_increment}"
    elif semantic_level == "minor":
        version_increment = int(version_split[1]) + 1
        new_version = f"{version_split[0]}.{version_increment}.0"
    elif semantic_level == "major":
        version_increment = int(version_split[0]) + 1
        new_version = f"{version_increment}.0.0"
    else:
        raise ValueError("Semantic level must be 'patch', 'minor', or 'major")

    return new_version


def update_workflow_spec(semantic_level: str, tenants_shared_to: list) -> dict:
    """
    Update the workflow spec.

    Args:
        deploy_prod (str): The deployment environment (e.g., 'production', 'staging').

    Returns:
        dict: The updated workflow spec.
    """
    previous_specs = [
        i
        for i in block_client.list_specs()
        if i["metadata"]["name"] == "build-metis-workflow"
    ]

    if len(previous_specs) > 1:
        raise ValueError("More than one workflow spec found")

    previous_spec = previous_specs[0]
    previous_version = previous_spec["latestRelease"]["version"]

    new_version = increment_version(previous_version, semantic_level)

    workflow_spec = build_workflow_spec()
    workflow_spec["body"]["config"]["images"]["metis-workflow-image"][
        "version"
    ] = new_version

    final_spec = {
        "config": workflow_spec["body"]["config"],
        "release": {"version": new_version},
    }

    updated_block_id = block_client.create_spec_release(
        spec_id=previous_spec["id"],
        body=final_spec,
        artifact=workflow_spec["artifact"],
        parameters=workflow_spec["parameters"],
    )

    try:
        block_client.update_spec_metadata(
            spec_id=previous_spec["id"],
            body={"scope": "shared", "tenants": tenants_shared_to},
        )
    except Exception as e:
        if "No changes detected" not in str(e):
            logging.warning(e)

    return updated_block_id


def build_wepapp_spec():
    """
    Build the web app specification.

    Args:
        deploy_prod (str): The deployment environment (e.g., 'production', 'staging').

    Returns:
        dict: The web app specification.
    """
    web_app_spec = {
        "body": {
            "version": "1",
            "kind": "webapp",
            "metadata": {
                "name": "metis-webapp",
                "title": "Model Explainability Webapp",
                "summary": "Webapp to explain model predictions",
                "description": "Model Explainability Webapp",
                "imageUrl": "https://upload.wikimedia.org/wikipedia/commons/thumb/5/53/Winged_goddess_Louvre_F32.jpg/440px-Winged_goddess_Louvre_F32.jpg",
            },
            "release": {"version": "0.0.1"},
            "config": {
                "image": {
                    "context": ".",
                    "dockerfile": "Dockerfile.webapp",
                    "useCache": False,
                    "version": "0.0.1",
                    "secrets": ["GITHUB_TOKEN", "API_KEY"],
                    "buildArguments": {
                        "CONFIG_REPOSITORY": "@param:config_repository",
                        "CONFIG_FILE_NAME": "@param:config_file_name",
                        "GLOSSARY_FILE": "@param:glossary_file",
                    },
                },
                "resources": {"instanceTypeId": 51},
                "sessionStickiness": True,
            },
        },
        "parameters": {
            "build": [
                {
                    "name": "config_repository",
                    "required": True,
                    "type": "string",
                    "defaultValue": "nimbus-labs/ds-financeco-audiences",
                },
                {
                    "name": "config_file_name",
                    "required": True,
                    "type": "string",
                    "defaultValue": "webapps/model_explain/config.yaml",
                },
                {
                    "name": "glossary_file",
                    "required": False,
                    "type": "string",
                    "defaultValue": "webapps/model_explain/glossary.yaml",
                },
            ],
        },
        "artifact": {"path": "."},
    }
    return web_app_spec


def update_webapp_spec(semantic_level: str, tenants_shared_to: list) -> dict:
    previous_specs = [
        i for i in block_client.list_specs() if i["metadata"]["name"] == "metis-webapp"
    ]

    if len(previous_specs) > 1:
        raise ValueError("More than one webapp spec found")

    previous_spec = previous_specs[0]
    previous_version = previous_spec["latestRelease"]["version"]

    new_version = increment_version(previous_version, semantic_level)

    webapp_spec = build_wepapp_spec()

    webapp_spec["body"]["config"]["image"]["version"] = new_version

    final_spec = {
        "config": webapp_spec["body"]["config"],
        "release": {"version": new_version},
    }

    updated_block_id = block_client.create_spec_release(
        spec_id=previous_spec["id"],
        body=final_spec,
        artifact=webapp_spec["artifact"],
        parameters=webapp_spec["parameters"],
    )

    try:
        block_client.update_spec_metadata(
            spec_id=previous_spec["id"],
            body={"scope": "shared", "tenants": tenants_shared_to},
        )
    except Exception as e:
        if "No changes detected" not in str(e):
            logging.warning(e)

    return updated_block_id


def build_app_spec(
    workflow_spec_id: int,
    webapp_spec_id: int,
):
    """
    Build the app specification.

    Args:
        workflow_spec_id (int): The ID of the workflow specification.
        webapp_spec_id (int): The ID of the web app specification.

    Returns:
        dict: The app specification.
    """
    app_spec = {
        "body": {
            "version": "1",
            "kind": "app",
            "metadata": {
                "name": "explainability-app",
                "title": "Model Explainability Press App",
                "summary": "App to explain model predictions",
                "description": "Model Explainability App",
                "descriptionContentType": "text/markdown",
                "imageUrl": "https://upload.wikimedia.org/wikipedia/commons/thumb/5/53/Winged_goddess_Louvre_F32.jpg/440px-Winged_goddess_Louvre_F32.jpg",
            },
            "release": {
                "version": "0.0.1",
                "notes": "Initial release",
            },
            "config": [
                {
                    "id": workflow_spec_id,
                    "release": {"version": "0.0.1"},
                },
                {
                    "id": webapp_spec_id,
                    "release": {"version": "0.0.1"},
                },
            ],
        },
    }

    return app_spec


def update_app_spec(
    semantic_level: str,
    tenants_shared_to: list,
) -> dict:

    previous_specs = [
        i
        for i in app_client.list_specs()
        if i["metadata"]["name"] == "explainability-app"
    ]

    if len(previous_specs) > 1:
        raise ValueError("More than one app spec found")

    previous_spec = previous_specs[0]
    previous_version = previous_spec["latestRelease"]["version"]

    new_version = increment_version(previous_version, semantic_level)

    webapp_spec_id = [
        i for i in block_client.list_specs() if i["metadata"]["name"] == "metis-webapp"
    ][0]["id"]

    workflow_spec_id = [
        i
        for i in block_client.list_specs()
        if i["metadata"]["name"] == "build-metis-workflow"
    ][0]["id"]

    app_spec = build_app_spec(
        workflow_spec_id,
        webapp_spec_id,
    )

    app_spec["body"]["release"]["version"] = new_version

    for i in range(len(app_spec["body"]["config"])):
        app_spec["body"]["config"][i]["release"]["version"] = new_version

    final_spec = {
        "config": app_spec["body"]["config"],
        "release": {"notes": "Praise the sun, another release", "version": new_version},
    }

    app_client.create_spec_release(
        spec_id=previous_spec["id"],
        body=final_spec,
    )

    try:
        app_client.update_spec_metadata(
            spec_id=previous_spec["id"],
            body={"scope": "shared", "tenants": tenants_shared_to},
        )
    except Exception as e:
        if "No changes detected" not in str(e):
            logging.warning(e)


def deploy_model_explain_blocks(tenants_shared_to: list) -> None:
    """
    Create images and workflows.

    Parameters
    ----------
    deploy_prod: str
        Whether to skip the web app deployment.
    """
    with contextlib.suppress(Exception):
        purge = [".ipynb_checkpoints", "file_system_backend", "iframe_figures"]
        for purge_me in purge:
            for path in glob.glob(f"**/{purge_me}", recursive=True):
                logging.warning(path)
                shutil.rmtree(path)

    check_key_in_env("API_KEY")
    check_key_in_env("GITHUB_TOKEN")

    workflow_spec = build_workflow_spec()
    block_client.create_spec(
        body=workflow_spec["body"],
        parameters=workflow_spec["parameters"],
        artifact=workflow_spec["artifact"],
        scope="shared",
        tenants=tenants_shared_to,
    )

    webapp_spec = build_wepapp_spec()
    block_client.create_spec(
        body=webapp_spec["body"],
        parameters=webapp_spec["parameters"],
        artifact=webapp_spec["artifact"],
        scope="shared",
        tenants=tenants_shared_to,
    )


def deploy_model_explain_app(tenants_shared_to: list) -> None:
    webapp_spec_id = [
        i for i in block_client.list_specs() if i["metadata"]["name"] == "metis-webapp"
    ][0]["id"]

    workflow_spec_id = [
        i
        for i in block_client.list_specs()
        if i["metadata"]["name"] == "build-metis-workflow"
    ][0]["id"]

    app_spec = build_app_spec(workflow_spec_id, webapp_spec_id)
    app_client.create_spec(
        body=app_spec["body"],
        scope="shared",
        tenants=tenants_shared_to,
    )


def check_key_in_env(key_name: str) -> None:
    """
    Checks if the given key exists in the environment variables.

    Parameters:
        key_name (str): The name of the key to check.

    Returns:
        None: This function does not return anything.

    Raises:
        RuntimeError: If the key does not exist in the environment variables.

    """
    if key_name not in os.environ:
        msg = f"{key_name} must be set as an environment variable"
        raise RuntimeError(msg)
