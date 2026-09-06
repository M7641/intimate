"""Disposable Postgres container for data-backed integration tests.

Spins a throwaway `postgres` container via Docker, applies versioned seed SQL
(`seeds/*.sql`, schema + fixtures, in filename order), and hands back a DSN. The
container is expensive to start (~1-2s) but cheap to reset, so boot it once per
session and TRUNCATE + re-seed between tests for isolation.

House style: we shell out to `docker` rather than depend on a container SDK — no
extra moving parts beyond `psycopg`. Podman works too: it's argv-compatible, so
set `PG_HARNESS_RUNTIME=podman`.

Usage (see the matching conftest in references/integration.md):
    pg = PgContainer.start(seeds_dir="tests/seeds")
    conn = psycopg.connect(pg.dsn)
    ...
    pg.reset()   # between tests: TRUNCATE every seeded table, re-apply fixtures
    pg.stop()
"""

from __future__ import annotations

import os
import subprocess
import time
from dataclasses import dataclass, field
from pathlib import Path

import psycopg

DEFAULT_IMAGE = "postgres:16-alpine"
RUNTIME = os.getenv("PG_HARNESS_RUNTIME", "docker")
# The container's own password/db/user. Values are throwaway — the container is
# never exposed beyond the test run.
PASSWORD = "test"
DB = "test"
USER = "postgres"


@dataclass
class PgContainer:
    """A running, seeded Postgres container. Use :meth:`start` as the entry point."""

    container_id: str
    port: int
    seed_files: list[Path] = field(default_factory=list)

    @property
    def dsn(self) -> str:
        return f"postgresql://{USER}:{PASSWORD}@localhost:{self.port}/{DB}"

    @classmethod
    def start(
        cls,
        *,
        image: str = DEFAULT_IMAGE,
        seeds_dir: str | os.PathLike[str] | None = "tests/seeds",
        timeout: float = 30.0,
    ) -> "PgContainer":
        """Run Postgres detached on a random host port, wait until it accepts
        connections, then apply every `*.sql` under `seeds_dir` in sorted order.

        Raises ``RuntimeError`` if the runtime rejects the run and
        ``TimeoutError`` if the server never becomes ready.
        """
        # `-p 0:5432` lets the host kernel pick a free port, so parallel suites
        # and developer machines never collide on 5432.
        run = subprocess.run(
            [
                RUNTIME,
                "run",
                "-d",
                "--rm",
                "-e",
                f"POSTGRES_PASSWORD={PASSWORD}",
                "-e",
                f"POSTGRES_DB={DB}",
                "-p",
                "0:5432",
                image,
            ],
            capture_output=True,
            text=True,
        )
        if run.returncode != 0:
            raise RuntimeError(f"failed to start Postgres: {run.stderr.strip()}")
        container_id = run.stdout.strip()

        try:
            port = cls._published_port(container_id)
            seed_files = sorted(Path(seeds_dir).glob("*.sql")) if seeds_dir else []
            instance = cls(container_id=container_id, port=port, seed_files=seed_files)
            instance._wait_until_ready(timeout=timeout)
            instance._apply(seed_files)
        except Exception:
            # Never leak a container if seeding/startup fails partway.
            subprocess.run([RUNTIME, "stop", container_id], capture_output=True)
            raise
        return instance

    @staticmethod
    def _published_port(container_id: str) -> int:
        out = subprocess.run(
            [RUNTIME, "port", container_id, "5432"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        # e.g. "0.0.0.0:54321" (may be multiple lines for v4/v6) -> 54321
        return int(out.splitlines()[0].rsplit(":", 1)[1])

    def _wait_until_ready(self, *, timeout: float) -> None:
        deadline = time.monotonic() + timeout
        last_error: Exception | None = None
        while time.monotonic() < deadline:
            try:
                with psycopg.connect(self.dsn, connect_timeout=2):
                    return
            except psycopg.OperationalError as exc:  # not accepting yet
                last_error = exc
            time.sleep(0.25)
        raise TimeoutError(f"Postgres not ready in {timeout}s: {last_error}")

    def _apply(self, files: list[Path]) -> None:
        if not files:
            return
        with psycopg.connect(self.dsn, autocommit=True) as conn:
            for path in files:
                conn.execute(path.read_text())

    def reset(self) -> None:
        """Wipe data and re-apply the seed files, without restarting the container.

        TRUNCATE ... RESTART IDENTITY CASCADE clears every table in the public
        schema (and resets sequences) far faster than dropping the container.
        Re-applying the seeds restores a known fixture set for the next test.
        """
        with psycopg.connect(self.dsn, autocommit=True) as conn:
            tables = conn.execute(
                "SELECT tablename FROM pg_tables WHERE schemaname = 'public'"
            ).fetchall()
            if tables:
                names = ", ".join(f'"{t[0]}"' for t in tables)
                conn.execute(f"TRUNCATE {names} RESTART IDENTITY CASCADE")
        self._apply(self.seed_files)

    def stop(self) -> None:
        # Started with --rm, so stop also removes the container.
        subprocess.run([RUNTIME, "stop", self.container_id], capture_output=True)

    def __enter__(self) -> "PgContainer":
        return self

    def __exit__(self, *exc: object) -> None:
        self.stop()
