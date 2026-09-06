from pathlib import Path
import requests
import os

from pure.logging import NimbusLogger
from ouroboros.images import deploy_image
from ouroboros.types import Artifact

logger = NimbusLogger(name=__name__).logger


class Workflows:
    base_url = "https://service.nimbus.example/workflows/api/v1"

    def list_workflows(self) -> list:
        response = requests.get(
            f"{self.base_url}/workflows/",
            headers={"Authorization": os.getenv("API_KEY")},
            timeout=10,
            params={
                "pageSize": None,
                "workflowStatus": None,
                "lastExecutionStatus": None,
                "lastModifiedBy": None,
                "searchTerm": None,
            },
        )

        if response.status_code != 200:
            msg = f"Error listing workflows: {response.status_code} - {response.text}"
            raise RuntimeError(msg)

        return response.json().get("workflows", [])

    def get_workflow_details(self, workflow_id: int) -> dict:
        response = requests.get(
            f"{self.base_url}/workflows/{workflow_id}",
            headers={"Authorization": os.getenv("API_KEY")},
            timeout=10,
        )
        if response.status_code != 200:
            msg = f"Error getting workflow details: {response.status_code} - {response.text}"
            raise RuntimeError(msg)
        return response.json()

    def create_workflow(self, spec: dict) -> dict:
        response = requests.post(
            f"{self.base_url}/workflows",
            json=spec,
            headers={
                "Authorization": os.getenv("API_KEY"),
                "Content-Type": "application/json",
            },
            timeout=30,
        )

        if response.status_code != 201:
            msg = f"Error creating workflow: {response.status_code} - {response.text}"
            raise RuntimeError(msg)

        return response.json()

    def update_workflow(self, spec: dict) -> dict:
        response = requests.put(
            f"{self.base_url}/workflows/{spec['id']}",
            json=spec,  # Still required Json over data even with content-type set
            headers={
                "Authorization": os.getenv("API_KEY"),
                "Content-Type": "application/json",
            },
            timeout=30,
        )

        if response.status_code != 200:
            msg = f"Error updating workflow: {response.status_code} - {response.text}"
            raise RuntimeError(msg)

        return response.json()

    def create_or_update_workflow(self, spec: dict) -> dict:
        workflows = self.list_workflows()
        existing_workflow = next((w for w in workflows if w["name"] == spec["name"]), None)

        if existing_workflow:
            spec["id"] = existing_workflow["id"]
            return self.update_workflow(spec)
        else:
            return self.create_workflow(spec)


def deploy_workflows(
    workflow_name: str,
    target: str,
    workflow_specs: list[dict] | dict,
    dockerfile_path: str | Path = "Dockerfile.workflow",
    build_args: list[dict] | None = None,
    build_secrets: list | None = None,
    version: str = "v1",
    artifact: Artifact | None = None
) -> None:

    if isinstance(workflow_specs, dict):
        workflow_specs = [workflow_specs]

    if isinstance(dockerfile_path, Path):
        dockerfile_path = dockerfile_path.relative_to(Path.cwd())
        dockerfile_path = str(dockerfile_path)

    if artifact is None:
        artifact = {
            "path": ".",
            "ignore_files": [".dockerignore"],
        }

    workflow_client = Workflows()
    workflow_image_id, _ = deploy_image(
        body={
            "name": workflow_name + "-" + version + "-" + target + "-image",
            "type": "workflow",
            "buildDetails": {
                "source": "upload",
                "useCache": True,
                "context": ".",
                "dockerfilePath": dockerfile_path,
                "buildArguments": build_args or {},
                "secrets": build_secrets or [],
            },
        },
        artifact=artifact,
    )

    logger.info(
        "Workflow image deployed successfully with ID: %s",
        workflow_image_id,
    )

    for workflow_spec in workflow_specs:
        for step in workflow_spec.get("steps", {}).values():
            step["imageId"] = workflow_image_id

        workflow_client.create_or_update_workflow(spec=workflow_spec)

        logger.info(
            "Workflow %s deployed successfully with image ID %s",
            workflow_spec["name"],
            workflow_image_id,
        )
