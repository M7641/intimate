import os

import tenacity
import typer
from nimbus.resources import images, services

image_client: images.Image = images.get_client()
services_client: services.Service = services.get_client()


def main() -> None:
    check_key_in_env("API_KEY")
    deploy()

def deploy() -> None:
    webapp_image_id, webapp_version_id = deploy_webapp_image()
    deploy_services(
        image_id=webapp_image_id,
        version_id=webapp_version_id,
        name="frontend",
        service_type="web-app"
    )

def deploy_webapp_image() -> tuple[int, int]:
    image_name = "frontend"
    dockerfile_body = {
        "name": image_name,
        "type": "webapp",
        "buildDetails": {
            "source": "upload",
            "useCache": False,
            "context": ".",
            "dockerfilePath": "Dockerfile",
            "buildArguments": [],
        },
    }

    response = image_client.create_or_update_image_version(
        body=dockerfile_body,
        artifact={"path": ".", "ignore_files": [".dockerignore"]},
    )

    if any(word not in response for word in ["buildId", "imageId", "versionId"]):
        msg = f"Error updating image - got {response}"
        raise RuntimeError(msg)

    purge_old_images(image_name)

    return response["imageId"], response["versionId"]


@tenacity.retry(wait=tenacity.wait_fixed(60), stop=tenacity.stop_after_attempt(5))
def deploy_services(
    image_id: int,
    version_id: int,
    name: str,
    service_type: str,
) -> str:
    api_key = os.environ["API_KEY"]
    image_details = image_client.describe_image(image_id=image_id)
    status = image_details["latestVersion"]["lastBuildStatus"]

    if status == "failed":
        msg = "Image build failed \U0001f97a"
        raise RuntimeError(msg)

    if status == "building":
        typer.echo(f"Waiting for `{image_details['name']}` image to build.")
        image_details = image_client.describe_image(image_id=image_id)
        status = image_details["latestVersion"]["lastBuildStatus"]

    typer.echo(f"\nDeploying `{name}` web app.")

    body = {
        "name": name,
        "title": "Place holder",
        "imageDetails": {"imageId": image_id, "versionId": version_id},
        "parameters": {"env": {"API_KEY": api_key}},
        "description": "This is a place holder",
        "serviceType": service_type,
        "resources": {"instanceTypeId": 52},
        "scaleToZero": False,
    }
    response = services_client.create_or_update_service(body=body)

    if "id" not in response:
        msg = f"Error creating web app - got {response}"
        raise RuntimeError(msg)

    typer.echo(typer.style("OK. ✨", fg=typer.colors.GREEN))

    return response["id"]


def check_key_in_env(env_name: str) -> None:
    if not os.getenv(env_name, None):
        msg = f"{env_name} must be set as an environment variable"
        raise RuntimeError(msg)

def purge_old_images(image_name: str) -> None:
    typer.echo("Purging old images.")

    image_id = [i["id"] for i in image_client.list_images() if i["name"] in image_name][
        0
    ]
    image_versions = [i for i in image_client.list_image_versions(image_id)]

    if len(image_versions) > 2:
        remove_most_recent = sorted([i["createdAt"] for i in image_versions])
        versions_to_pruge = [
            i for i in image_versions if i["createdAt"] in remove_most_recent[:-3]
        ]
        for i in versions_to_pruge:
            image_client.delete_version(image_id=image_id, version_id=i["id"])
    typer.echo("Done.")

if __name__ == "__main__":
    typer.run(main)
