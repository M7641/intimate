import json

import typer
from rich.console import Console

from ouroboros.images import Images
from ouroboros.service import Services
from ouroboros.tenant import Tenant
from ouroboros.workflows import Workflows

app = typer.Typer()
console = Console()


@app.command("list-services")
def list_services(
    detailed: bool = typer.Option(False, "--detailed", "-d", help="Show detailed info"),
    save: bool = typer.Option(False, "--save", "-s", help="Save to file"),
) -> None:
    """
    List deployed services

    Examples:
        uv run ouroboros list-services
    """

    services_client = Services()
    platform_services = services_client.list_services()

    services = []
    for service in platform_services:
        if detailed:
            service_details = services_client.get_service_details(service["id"])
            services.append(service_details)
        else:
            services.append(service)

    if save:
        with open("services.json", "w") as f:
            json.dump(services, f, indent=4)
        console.print("Services saved to services.json")
    else:
        for service in services:
            console.print(service)


@app.command("describe-service")
def describe_service(
    service_id: str = typer.Argument(..., help="The ID of the service to describe"),
    save: bool = typer.Option(False, "--save", "-s", help="Save to file"),
) -> None:
    """
    Describe a specific service by ID

    Examples:
        uv run ouroboros describe-service <service_id>
    """

    services_client = Services()
    service_details = services_client.get_service_details(service_id)
    console.print(service_details)

    if save:
        with open(f"service_{service_id}.json", "w") as f:
            json.dump(service_details, f, indent=4)
        console.print(f"Service details saved to service_{service_id}.json")


@app.command("list-workflows")
def list_workflows(
    detailed: bool = typer.Option(False, "--detailed", "-d", help="Show detailed info"),
    save: bool = typer.Option(False, "--save", "-s", help="Save to file"),
) -> None:
    """
    List deployed workflows

    Examples:
        uv run ouroboros list-workflows
    """

    workflows_client = Workflows()
    platform_workflows = workflows_client.list_workflows()

    workflows = []
    for wf in platform_workflows:
        if detailed:
            workflow_details = workflows_client.get_workflow_details(wf["id"])
            workflows.append(workflow_details)
        else:
            workflows.append(wf)

    if save:
        with open("workflows.json", "w") as f:
            json.dump(workflows, f, indent=4)
        console.print("Workflows saved to workflows.json")
    else:
        for wf in workflows:
            console.print(wf)


@app.command("describe-workflow")
def describe_workflow(
    workflow_id: int = typer.Argument(..., help="The ID of the workflow to describe"),
    save: bool = typer.Option(False, "--save", "-s", help="Save to file"),
) -> None:
    """
    Describe a specific workflow by ID

    Examples:
        uv run ouroboros describe-workflow <workflow_id>
    """

    workflows_client = Workflows()
    workflow_details = workflows_client.get_workflow_details(workflow_id)
    console.print(workflow_details)

    if save:
        with open(f"workflow_{workflow_id}.json", "w") as f:
            json.dump(workflow_details, f, indent=4)
        console.print(f"Workflow details saved to workflow_{workflow_id}.json")


@app.command("list-images")
def list_images(
    detailed: bool = typer.Option(False, "--detailed", "-d", help="Show detailed info"),
    save: bool = typer.Option(False, "--save", "-s", help="Save to file"),
) -> None:
    """
    List deployed images

    Examples:
        uv run ouroboros list-images
    """

    images_client = Images()
    platform_images = images_client.list_images()

    images = []
    for img in platform_images:
        if detailed:
            image_details = images_client.describe_image(img["id"])
            images.append(image_details)
        else:
            images.append(img)

    if save:
        with open("images.json", "w") as f:
            json.dump(images, f, indent=4)
        console.print("Images saved to images.json")
    else:
        for img in images:
            console.print(img)


@app.command("describe-image")
def describe_image(
    image_id: int = typer.Argument(..., help="The ID of the image to describe"),
    save: bool = typer.Option(False, "--save", "-s", help="Save to file"),
) -> None:
    """
    Describe a specific image by ID

    Examples:
        uv run ouroboros describe-image <image_id>
    """

    images_client = Images()
    image_details = images_client.describe_image(image_id)
    console.print(image_details)

    if save:
        with open(f"image_{image_id}.json", "w") as f:
            json.dump(image_details, f, indent=4)
        console.print(f"Image details saved to image_{image_id}.json")


@app.command("instance-options")
def instance_options(
    entity_type: str = typer.Argument(
        ...,
        help="Entity type (e.g. workflow, webapp, workspace)",
    ),
) -> None:
    """
    Show available instance types and sizes for a given entity type.

    Examples:
        uv run ouroboros instance-options workflow
        uv run ouroboros instance-options webapp
        uv run ouroboros instance-options api-deployment
    """
    tenant = Tenant()
    tenant.print_instance_options(entity_type)


if __name__ == "__main__":
    app()
