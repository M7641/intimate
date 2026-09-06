"""Manage the lifecycle of a local MiniStack container.

MiniStack (https://github.com/ministackorg/ministack) is a free, MIT-licensed
local AWS emulator. It exposes 56+ AWS services on a single port (4566) and is
drop-in compatible with boto3 / the AWS CLI.

This module spins the emulator up in Docker, waits for it to become healthy,
and exposes helpers to reset state between tests. It deliberately shells out to
``docker`` rather than depending on the docker SDK so the pilot has no extra
moving parts beyond boto3 and psycopg.
"""

from __future__ import annotations

import subprocess
import time
import urllib.error
import urllib.request
from dataclasses import dataclass

DEFAULT_IMAGE = "ministackorg/ministack"
DEFAULT_PORT = 4566
# MiniStack ignores credentials but boto3 still requires *some* value.
TEST_CREDENTIALS = {
    "aws_access_key_id": "test",
    "aws_secret_access_key": "test",
    "region_name": "us-east-1",
}


@dataclass
class MiniStack:
    """A running MiniStack container.

    Use :meth:`start` as the entry point; it returns a started instance. Call
    :meth:`reset` between tests to wipe all emulated state without paying the
    container restart cost, and :meth:`stop` when finished.
    """

    container_id: str
    port: int = DEFAULT_PORT

    @property
    def endpoint_url(self) -> str:
        return f"http://localhost:{self.port}"

    @classmethod
    def start(
        cls,
        *,
        image: str = DEFAULT_IMAGE,
        port: int = DEFAULT_PORT,
        timeout: float = 30.0,
    ) -> "MiniStack":
        """Start MiniStack in a detached Docker container and wait for health.

        Raises ``TimeoutError`` if the health endpoint does not respond within
        ``timeout`` seconds, and ``RuntimeError`` if Docker rejects the run.
        """
        run = subprocess.run(
            ["docker", "run", "-d", "--rm", "-p", f"{port}:4566", image],
            capture_output=True,
            text=True,
        )
        if run.returncode != 0:
            raise RuntimeError(f"failed to start MiniStack: {run.stderr.strip()}")

        instance = cls(container_id=run.stdout.strip(), port=port)
        instance._wait_until_healthy(timeout=timeout)
        return instance

    def _wait_until_healthy(self, *, timeout: float) -> None:
        # MiniStack exposes an internal health endpoint that flips to 200 once
        # every emulated service is wired up.
        health = f"{self.endpoint_url}/_ministack/health"
        deadline = time.monotonic() + timeout
        last_error: Exception | None = None
        while time.monotonic() < deadline:
            try:
                with urllib.request.urlopen(health, timeout=2) as resp:
                    if resp.status == 200:
                        return
            except (urllib.error.URLError, ConnectionError, OSError) as exc:
                last_error = exc
            time.sleep(0.25)
        raise TimeoutError(
            f"MiniStack did not become healthy in {timeout}s: {last_error}"
        )

    def reset(self) -> None:
        """Wipe all emulated state without restarting the container.

        Ideal for ``setUp`` / fixtures so each test sees a clean environment.
        """
        req = urllib.request.Request(
            f"{self.endpoint_url}/_ministack/reset", method="POST"
        )
        with urllib.request.urlopen(req, timeout=10):
            pass

    def stop(self) -> None:
        # The container was started with --rm, so a stop also removes it.
        subprocess.run(
            ["docker", "stop", self.container_id],
            capture_output=True,
            text=True,
        )

    def __enter__(self) -> "MiniStack":
        return self

    def __exit__(self, *exc: object) -> None:
        self.stop()
