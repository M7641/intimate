import os
import time
from pathlib import Path

import requests
from pure.logging import NimbusLogger

from ouroboros.images import Images, deploy_image
from ouroboros.types import Artifact

logger = NimbusLogger(__name__).logger


class Services:
    base_url = "https://service.nimbus.example/webapps/api/v1"

    def list_services(self) -> list:
        response = requests.get(
            f"{self.base_url}/webapps/",
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
            msg = f"Error listing services: {response.status_code} - {response.text}"
            raise RuntimeError(msg)

        return response.json().get("webapps", [])

    def get_service_details(self, service_id: str) -> dict:
        response = requests.get(
            f"{self.base_url}/webapps/{service_id}",
            headers={"Authorization": os.getenv("API_KEY")},
            timeout=10,
        )

        if response.status_code != 200:
            msg = f"Error getting service details: {response.status_code} - {response.text}"
            raise RuntimeError(msg)

        return response.json()

    def create_service(self, body: dict) -> tuple[dict, int]:
        response = requests.post(
            f"{self.base_url}/webapps",
            json=body,
            headers={
                "Authorization": os.getenv("API_KEY"),
                "Content-Type": "application/json",
            },
            timeout=30,
        )

        if response.status_code != 202:
            msg = f"Error creating service: {response.status_code} - {response.text}"
            raise RuntimeError(msg)

        return response.json(), response.status_code

    def update_service(self, body: dict, service_id: int) -> tuple[dict, int]:
        response = requests.patch(
            f"{self.base_url}/webapps/{service_id}",
            json=body,  # Still required Json over data even with content-type set
            headers={
                "Authorization": os.getenv("API_KEY"),
                "Content-Type": "application/json",
            },
            timeout=30,
        )

        if response.status_code != 202:
            msg = f"Error updating service: {response.status_code} - {response.text}"
            raise RuntimeError(msg)

        return response.json(), response.status_code

    def create_or_update_service(self, body: dict) -> tuple[dict, int]:
        services = self.list_services()
        service = next((s for s in services if s["name"] == body["name"]), None)

        if service:
            return self.update_service(body, service["id"])
        else:
            return self.create_service(body)


def build_service(
    service_name: str,
    image_id: int,
    version_id: int,
    target: str,
    num_instances: int = 1,
    service_type: str = "webapp",
    scale_to_zero: bool = True,
    resources: dict | None = None,
):
    """
    Further:
        1. More flexibility on choosing instance types based on service type.
        For that, we will need to move the list intsance options over and make that
        a better experience on what you can use.
    """
    services_client = Services()

    if resources is None:
        if service_type == "webapp":
            resources = {"instanceTypeId": 47}
        elif service_type == "api":
            resources = {"instanceTypeId": 23}
        else:
            raise ValueError(
                "Either resources or a service_type of 'webapp' or 'api' must be provided"
            )

    if service_type == "webapp":
        service_type = "web-app"

    response, status_code = services_client.create_or_update_service(
        body={
            "name": service_name,
            "title": f"{service_name.replace('_', '-').capitalize()} Service",
            "serviceType": service_type,
            "imageDetails": {"imageId": image_id, "versionId": version_id},
            "description": service_type.replace("-", " ").capitalize(),
            "resources": resources,
            "minInstances": num_instances,
            "scaleToZero": scale_to_zero,
            "tags": [{"name": target}],
        }
    )

    if status_code != 202:
        msg = f"Error creating web app - got {status_code} - {response.get('message')}"
        raise RuntimeError(msg)
    else:
        logger.info("Web app created or updated successfully: %s", response["id"])
        return response["id"]


def deploy_service(
    service_name: str,
    service_type: str = "webapp",
    target: str = "test",
    build_args: list[dict] | None = None,
    build_secrets: list | None = None,
    dockerfile_path: str | Path = "Dockerfile.webapp",
    version: str = "v1",
    artifact: Artifact | None = None,
    scale_to_zero: bool = False,
    num_instances: int = 1,
    resources: dict | None = None,
) -> None:
    image_client = Images()

    service_name = service_name.replace("_", "-").lower() + f"-{version}-{target}"
    logger.info("Deploying the service called: %s", service_name)

    if isinstance(dockerfile_path, Path):
        dockerfile_path = dockerfile_path.relative_to(Path.cwd())
        dockerfile_path = str(dockerfile_path)

    if artifact is None:
        artifact = {
            "path": ".",
            "ignore_files": [".dockerignore"],
        }

    image_id, version_id = deploy_image(
        body={
            "name": service_name + "-image",
            "type": service_type,
            "buildDetails": {
                "source": "upload",
                "useCache": True,
                "context": ".",
                "dockerfilePath": dockerfile_path,
                "buildArguments": build_args or [],
                "secrets": build_secrets or [],
            },
        },
        artifact=artifact,
    )

    starting_time = time.time()
    while True:
        image_details = image_client.describe_image(image_id=image_id)

        if not image_details:
            raise RuntimeError("No image details found")

        image_build_status = image_details.get("latestVersion", {}).get(
            "lastBuildStatus"
        )

        if image_build_status == "success":
            logger.info("Image version is now available")
            break
        elif image_build_status == "failed":
            raise RuntimeError("Image build failed")
        elif image_build_status == "building":
            time.sleep(15)
        elif time.time() - starting_time > 600:
            raise TimeoutError("Timed out waiting for image to be ready")

    logger.info("Image deployed with ID: %s and version ID: %s", image_id, version_id)

    build_service(
        service_name=service_name,
        service_type=service_type,
        image_id=image_id,
        version_id=version_id,
        target=target,
        scale_to_zero=scale_to_zero,
        num_instances=num_instances,
        resources=resources,
    )
