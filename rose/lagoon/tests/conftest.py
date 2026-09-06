"""Pytest fixtures that spin MiniStack up once per session.

The container is expensive to start (~2s) but cheap to reset, so we boot it once
and call the ``/_ministack/reset`` endpoint before each test for isolation.

Tests are skipped automatically when Docker is unavailable, so the suite stays
green on machines without a container runtime.
"""

from __future__ import annotations

import shutil
import subprocess

import pytest

from lagoon.ministack import MiniStack


def _docker_available() -> bool:
    if shutil.which("docker") is None:
        return False
    probe = subprocess.run(["docker", "info"], capture_output=True)
    return probe.returncode == 0


requires_docker = pytest.mark.skipif(
    not _docker_available(), reason="Docker is not available"
)


@pytest.fixture(scope="session")
def ministack():
    if not _docker_available():
        pytest.skip("Docker is not available")
    instance = MiniStack.start()
    yield instance
    instance.stop()


@pytest.fixture
def fresh_ministack(ministack):
    """A MiniStack with clean state for the test that requests it."""
    ministack.reset()
    return ministack
