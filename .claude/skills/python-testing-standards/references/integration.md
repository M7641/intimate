# Integration tests against a real backing service (Python)

Some tests can't run against SQLite or a mock — they need a *real* Postgres (SQL
the code actually issues, server-side behaviour, fixtures that power a query) or a
*real* S3 API (object put/get/list). For those, **spin up a throwaway container,
seed it, assert, throw it away.**

House style: we shell out to `docker` rather than depend on a container SDK — no
extra moving parts beyond the client library (`psycopg`, `boto3`). Podman works
too (argv-compatible); set `PG_HARNESS_RUNTIME=podman`.

Three rules keep these a clean PR gate instead of a flaky one:

1. **Boot once, reset per test.** Starting a container is the slow part (~1-2s);
   resetting (TRUNCATE + re-seed) is milliseconds. Session-scoped fixture for the
   container, function-scoped for a clean dataset.
2. **Skip when Docker is absent**, so the suite stays green on machines without a
   runtime and the gate only bites in CI (where Docker is present). These stay in
   the normal `test` task — no separate moon task, no `runInCI: false`.
3. **Pin the image tag** (`postgres:16-alpine`, `adobe/s3mock:latest`→ a digest)
   so the gate is reproducible.

CI note: GitHub Actions has Docker on `ubuntu-latest`, so these run under the
normal `moon ci` affected `test` task with no extra setup.

## Disposable Postgres (data-backed tests)

Copy `templates/pg_harness.py` — a `PgContainer` helper that runs Postgres on a
random host port, waits until it accepts connections, and applies versioned seed
SQL. Dev-dep: `psycopg[binary]`.

**Seed from versioned `.sql` files**, not inline Python. Keep a `tests/seeds/` dir
whose files apply in filename order — schema first, fixtures second:

```
tests/seeds/
├── 01_schema.sql      # CREATE TABLE events (...);
└── 02_fixtures.sql    # INSERT INTO events VALUES (...);
```

This keeps the seed diffable in review, reusable outside tests, and shareable with
a Rust suite (same `.sql`). Inline `CREATE/INSERT` in a fixture is fine for data
specific to a single test; shared fixtures belong in `.sql`.

Wire it up with two fixtures — boot once, reset per test:

```python
# conftest.py
import shutil, subprocess, pytest, psycopg
from pg_harness import PgContainer

def _docker_available() -> bool:
    if shutil.which("docker") is None:
        return False
    return subprocess.run(["docker", "info"], capture_output=True).returncode == 0

requires_docker = pytest.mark.skipif(
    not _docker_available(), reason="Docker is not available")

@pytest.fixture(scope="session")
def pg():
    if not _docker_available():
        pytest.skip("Docker is not available")
    with PgContainer.start(seeds_dir="tests/seeds") as container:
        yield container

@pytest.fixture
def db(pg):
    """A connection to a freshly re-seeded database for the requesting test."""
    pg.reset()                       # TRUNCATE + re-apply fixtures: cheap isolation
    with psycopg.connect(pg.dsn) as conn:
        yield conn
```

```python
# events_test.py
from .conftest import requires_docker

@requires_docker
def test_regional_totals(db):
    with db.cursor() as cur:
        cur.execute("SELECT region, SUM(amount) FROM events GROUP BY region ORDER BY 1")
        assert cur.fetchall() == [("amer", 80), ("emea", 170)]
```

**Reset, don't restart.** For code that doesn't commit its own transactions,
wrapping each test in a transaction and rolling back is even faster — but
TRUNCATE+reseed is the robust default that survives internal commits.

## Mocking S3 — S3Mock container

`adobe/s3mock` (Apache-2.0) is a lightweight, S3-only mock — much smaller than a
full LocalStack when all you need is the object API. It serves the S3 API on port
**9090** (HTTP) / 9191 (HTTPS), pre-creates buckets from a comma-separated env var,
and only supports **path-style** access (`http://localhost:9090/bucket/key`), so
the client must be configured accordingly. Dev-dep: `boto3`.

```python
# conftest.py (additional fixtures)
import subprocess, time, urllib.request, boto3, pytest
from botocore.config import Config

S3MOCK_IMAGE = "adobe/s3mock"
INITIAL_BUCKETS = "test-bucket"

@pytest.fixture(scope="session")
def s3mock():
    if not _docker_available():
        pytest.skip("Docker is not available")
    run = subprocess.run(
        ["docker", "run", "-d", "--rm", "-p", "0:9090",
         "-e", f"COM_ADOBE_TESTING_S3MOCK_STORE_INITIAL_BUCKETS={INITIAL_BUCKETS}",
         S3MOCK_IMAGE],
        capture_output=True, text=True,
    )
    assert run.returncode == 0, run.stderr
    cid = run.stdout.strip()
    try:
        port = int(subprocess.run(["docker", "port", cid, "9090"],
                   capture_output=True, text=True, check=True
                   ).stdout.strip().splitlines()[0].rsplit(":", 1)[1])
        _wait_http(f"http://localhost:{port}/", timeout=30)
        yield f"http://localhost:{port}"
    finally:
        subprocess.run(["docker", "stop", cid], capture_output=True)

def _wait_http(url: str, *, timeout: float) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            urllib.request.urlopen(url, timeout=2)
            return
        except Exception:
            time.sleep(0.25)
    raise TimeoutError(f"S3Mock not ready in {timeout}s")

@pytest.fixture
def s3(s3mock):
    return boto3.client(
        "s3",
        endpoint_url=s3mock,
        aws_access_key_id="test", aws_secret_access_key="test",
        region_name="us-east-1",
        # S3Mock is path-style only — virtual-hosted addressing 404s.
        config=Config(s3={"addressing_style": "path"}),
    )
```

```python
# storage_test.py
from .conftest import requires_docker

@requires_docker
def test_put_then_list(s3):
    s3.put_object(Bucket="test-bucket", Key="events/1.json", Body=b"{}")
    keys = [o["Key"] for o in s3.list_objects_v2(Bucket="test-bucket").get("Contents", [])]
    assert keys == ["events/1.json"]
```

S3Mock starts clean each session; if a test needs isolation from others' objects,
delete the keys it created in a fixture teardown, or give each test its own bucket
(add it to `COM_ADOBE_TESTING_S3MOCK_STORE_INITIAL_BUCKETS`, or `create_bucket`).
