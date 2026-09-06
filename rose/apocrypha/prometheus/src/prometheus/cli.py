import os
from ouroboros.service import deploy_service
import typer

from pure.logging import NimbusLogger

app = typer.Typer()
logger = NimbusLogger(name=__name__).logger


@app.command()
def deploy() -> None:
    deploy_service(
        service_name="prometheus",
        service_type="webapp",
        target="dev",
        build_args=[{"name": "API_KEY", "value": os.getenv("API_KEY")}],
        dockerfile_path="Dockerfile.prometheus",
        version="v2",
        artifact={
            "path": "modules/prometheus",
            "ignore_files": [".dockerignore"],
        },
    )


if __name__ == "__main__":
    app()
