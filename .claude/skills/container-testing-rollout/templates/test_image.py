"""Runtime axis (Python): black-box integration test of the *production container
image*. Copy to `tests/test_image.py` and edit the port, wait line, and asserted
endpoints to match your image's contract.

A distroless/slim image is exercised over HTTP exactly as production runs it.
The `DockerContainer` is a context manager, so cleanup on ``__exit__`` plays the
same role Rust's ``Drop`` does — automatic teardown, no leaked containers.

Dev-deps to add (uv dev group):
    testcontainers
    httpx

Prerequisites: the image must be built (``moon run <project>:image-build``) and a
container engine must be reachable. When none is, the test skips cleanly, so it
never fails a machine without Podman/Docker. The ``moon run <project>:image-test``
wrapper sets DOCKER_HOST, TESTCONTAINERS_RYUK_DISABLED and IMAGE_REF for you
(see templates/moon-image-tasks.yml).
"""

import os
import shutil
import subprocess

import httpx
import pytest
from testcontainers.core.container import DockerContainer
from testcontainers.core.waiting_utils import wait_for_logs


def _engine_available() -> bool:
    """True when a container engine responds to ``info`` — the same skip guard our
    Postgres/S3 integration tests use, so the suite stays green without an engine."""
    for engine in ("podman", "docker"):
        if shutil.which(engine) is None:
            continue
        result = subprocess.run([engine, "info"], capture_output=True, check=False)
        if result.returncode == 0:
            return True
    return False


@pytest.mark.skipif(
    not _engine_available(), reason="no container engine (podman/docker) reachable"
)
def test_image_serves_its_endpoints() -> None:
    # Podman requires fully-qualified names, so the wrapper passes
    # IMAGE_REF=localhost/myapp:prod; the bare default suits Docker.
    image = os.environ.get("IMAGE_REF", "myapp:prod")

    with DockerContainer(image).with_exposed_ports(8000) as container:  # EDIT: port
        # EDIT: the exact line the app logs once startup is complete.
        wait_for_logs(container, "Application startup complete")
        port = container.get_exposed_port(8000)
        base = f"http://127.0.0.1:{port}"

        # EDIT: the endpoints that define your image's contract.
        for path, what in (("/health", "health endpoint"), ("/", "root")):
            resp = httpx.get(f"{base}{path}")
            assert resp.status_code == 200, f"{what} at {path} should return 200"
    # container stops on __exit__ — same automatic teardown as Rust's Drop.
