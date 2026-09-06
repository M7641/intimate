import os

from nimbus.resources import images, workflows

image_client = images.get_client()
workflow_client = workflows.get_client()


def return_image_id() -> int:
    image_id = [
        i for i in image_client.list_images() if "metis-workflow-image" in i["name"]
    ][0]["id"]

    return image_id


def deploy_workflow(spec: dict) -> int:
    response = workflow_client.create_or_update_workflow(spec)

    if "id" not in response:
        msg = f"Error creating workflow - got {response}"
        raise RuntimeError(msg)

    return response["id"]


def add_config_variables() -> list:
    """
    Generates a list of configuration variables based on the deploy_prod flag.

    Args:
        deploy_prod (str).

    Returns:
        list: A list of dictionaries representing the configuration variables.
            Each dictionary contains the keys "name" and "value" representing the
            variable name and its corresponding value.

    Example:
        >>> add_config_variables()
        [
            {"name": "GITHUB_TOKEN_GITHUB", "value": "XYZ"},
        ]
    """
    return [
        {"name": "GITHUB_TOKEN_GITHUB", "value": os.getenv("GITHUB_TOKEN")},
        {"name": "API_KEY_CUSTOM", "value": os.getenv("API_KEY")},
        {"name": "CONFIG_REPOSITORY", "value": os.getenv("CONFIG_REPOSITORY")},
        {"name": "CONFIG_FILE_NAME", "value": os.getenv("CONFIG_FILE_NAME")},
    ]


def deploy_workflow_image() -> int:

    dockerfile_body = {
        "name": "metis-build-shap-values-image",
        "type": "workflow",
        "buildDetails": {
            "source": "upload",
            "useCache": False,
            "context": ".",
            "dockerfilePath": "Dockerfile.workflow",
            "buildArguments": add_config_variables(),
        },
    }

    response = image_client.create_or_update_image_version(
        body=dockerfile_body,
        artifact={"path": "."},
    )

    if not all(key in response for key in ["buildId", "imageId", "versionId"]):
        msg = f"Error updating image - got {response}"
        raise RuntimeError(msg)

    image_id = response["imageId"]

    return image_id


def build_workflow_spec(
    model_configurations: list,
    workflow_image_id_main: int,
) -> dict:
    """
    Build workflow spec.

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
        "name": "metis-build-shap-values",
        "triggers": [
            {
                "cron": "0 1 * * 0",
            }
        ],
    }

    workflow_spec_models = {
        "pip-list": {
            "imageId": workflow_image_id_main,
            "resources": {"instanceTypeId": 20, "storage": "1GB"},
            "command": "pip list",
        }
    }

    for i in model_configurations:
        key = "run-model-explain-" + i.replace("_", "-")
        workflow_spec_models[key] = {
            "imageId": workflow_image_id_main,
            "resources": {"instanceTypeId": 26, "storage": "10GB"},
            "command": f"metis run-workflow --run-prod --model-name={i}",
            "parents": ["pip-list"],
        }

    workflow_spec["steps"] = workflow_spec_models

    return workflow_spec


def bootstrap_workflow(
    model_configurations: list,
) -> None:
    """
    using the base workflow image as well, I attempted to make a new one, but
    it failed with no clear error.
    """

    workflow_image_id_main = return_image_id()

    spec = build_workflow_spec(
        model_configurations=model_configurations,
        workflow_image_id_main=workflow_image_id_main,
    )

    deploy_workflow(spec=spec)
