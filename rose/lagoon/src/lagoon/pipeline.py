"""Example pipeline: stage data in S3, load it into a warehouse, query both.

Two stores are exercised against a single MiniStack container:

* **S3** — emulated natively by MiniStack, used as a landing/staging area.
* **Warehouse** — provisioned through MiniStack's **RDS** API, which boots a
  *real* PostgreSQL container and hands back a live connection endpoint.

  MiniStack does not emulate Amazon Redshift. We use RDS PostgreSQL as a stand-in
  because Redshift is wire-compatible with PostgreSQL (it began as a fork of
  PostgreSQL 8.0.2): the same ``psycopg`` driver and ``SELECT`` statements that
  work here also work against a real Redshift cluster, so the query code in this
  pilot ports across with only a connection-string change. See the README for
  the caveats (no ``COPY ... FROM 's3://...'``, no ``DISTKEY``/``SORTKEY``, etc.).
"""

from __future__ import annotations

import time
from dataclasses import dataclass

import boto3
import psycopg

from .ministack import TEST_CREDENTIALS, MiniStack


def s3_client(ministack: MiniStack):
    """Return a boto3 S3 client pointed at the local MiniStack."""
    return boto3.client("s3", endpoint_url=ministack.endpoint_url, **TEST_CREDENTIALS)


def rds_client(ministack: MiniStack):
    """Return a boto3 RDS client pointed at the local MiniStack."""
    return boto3.client("rds", endpoint_url=ministack.endpoint_url, **TEST_CREDENTIALS)


@dataclass
class WarehouseParams:
    """Connection details for the Postgres-backed warehouse."""

    host: str
    port: int
    dbname: str
    user: str
    password: str

    def dsn(self) -> str:
        return (
            f"host={self.host} port={self.port} dbname={self.dbname} "
            f"user={self.user} password={self.password}"
        )


def provision_warehouse(
    ministack: MiniStack,
    *,
    identifier: str = "warehouse",
    dbname: str = "analytics",
    username: str = "admin",
    password: str = "secret123",
    timeout: float = 60.0,
) -> WarehouseParams:
    """Provision a Postgres "warehouse" through MiniStack's RDS API.

    MiniStack starts a real Postgres container and returns its endpoint once the
    instance reaches the ``available`` state. We rewrite the reported host to
    ``localhost`` because the container's internal hostname is not resolvable
    from the test process.
    """
    rds = rds_client(ministack)
    rds.create_db_instance(
        DBInstanceIdentifier=identifier,
        Engine="postgres",
        DBName=dbname,
        MasterUsername=username,
        MasterUserPassword=password,
        AllocatedStorage=5,
        DBInstanceClass="db.t3.micro",
    )

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        described = rds.describe_db_instances(DBInstanceIdentifier=identifier)
        instance = described["DBInstances"][0]
        if instance["DBInstanceStatus"] == "available" and instance.get("Endpoint"):
            endpoint = instance["Endpoint"]
            return WarehouseParams(
                host="localhost",
                port=int(endpoint["Port"]),
                dbname=dbname,
                user=username,
                password=password,
            )
        time.sleep(0.5)
    raise TimeoutError(f"warehouse {identifier!r} not available within {timeout}s")


def connect_warehouse(
    params: WarehouseParams, *, timeout: float = 30.0
) -> psycopg.Connection:
    """Open a psycopg connection, retrying until the Postgres container accepts it."""
    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            return psycopg.connect(params.dsn())
        except psycopg.OperationalError as exc:
            last_error = exc
            time.sleep(0.5)
    raise TimeoutError(
        f"could not connect to warehouse within {timeout}s: {last_error}"
    )


# --- Example workload --------------------------------------------------------

SAMPLE_EVENTS = [
    {"id": 1, "region": "emea", "amount": 120},
    {"id": 2, "region": "amer", "amount": 80},
    {"id": 3, "region": "emea", "amount": 50},
]


def stage_to_s3(ministack: MiniStack, bucket: str = "staging") -> list[str]:
    """Create a bucket and stage one object per sample event. Returns the keys."""
    s3 = s3_client(ministack)
    s3.create_bucket(Bucket=bucket)
    keys: list[str] = []
    for event in SAMPLE_EVENTS:
        key = f"events/{event['id']}.json"
        s3.put_object(Bucket=bucket, Key=key, Body=_to_json(event))
        keys.append(key)
    return keys


def load_into_warehouse(conn: psycopg.Connection) -> None:
    """Create the target table and load the sample events into the warehouse."""
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS events (
                id     INTEGER PRIMARY KEY,
                region TEXT    NOT NULL,
                amount INTEGER NOT NULL
            )
            """
        )
        cur.executemany(
            "INSERT INTO events (id, region, amount) VALUES (%s, %s, %s) "
            "ON CONFLICT (id) DO NOTHING",
            [(e["id"], e["region"], e["amount"]) for e in SAMPLE_EVENTS],
        )
    conn.commit()


def query_s3(ministack: MiniStack, bucket: str = "staging") -> list[str]:
    """Query S3: list the staged object keys (sorted for determinism)."""
    s3 = s3_client(ministack)
    listing = s3.list_objects_v2(Bucket=bucket)
    return sorted(obj["Key"] for obj in listing.get("Contents", []))


def query_warehouse(conn: psycopg.Connection) -> list[tuple[str, int]]:
    """Query the warehouse: total amount per region (the kind of aggregation
    you would run on Redshift), ordered by region."""
    with conn.cursor() as cur:
        cur.execute(
            "SELECT region, SUM(amount) AS total "
            "FROM events GROUP BY region ORDER BY region"
        )
        return [(region, int(total)) for region, total in cur.fetchall()]


def _to_json(obj: dict) -> bytes:
    import json

    return json.dumps(obj).encode("utf-8")
