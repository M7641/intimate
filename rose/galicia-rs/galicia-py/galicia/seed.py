"""Génère des lots de données synthétiques pour simuler l'ingestion depuis l'OLTP.

Dans l'archi cible, ces lignes viendraient d'un CDC depuis RDS Postgres
(Debezium → Kinesis → un petit writer Python) ou d'un dump nocturne.
"""

from __future__ import annotations

import random
from datetime import datetime, timedelta, timezone

import pyarrow as pa

COUNTRIES = ["FR", "ES", "PT", "IT", "DE"]
CURRENCIES = {"FR": "EUR", "ES": "EUR", "PT": "EUR", "IT": "EUR", "DE": "EUR"}


def customers(n: int, *, seed: int = 0) -> pa.Table:
    rng = random.Random(seed)
    base = datetime(2024, 1, 1, tzinfo=timezone.utc)
    rows = [
        {
            "customer_id": i,
            "name": f"customer_{i:04d}",
            "country": rng.choice(COUNTRIES),
            "signup_at": base + timedelta(days=rng.randint(0, 600)),
        }
        for i in range(n)
    ]
    return pa.Table.from_pylist(
        rows,
        schema=pa.schema(
            [
                ("customer_id", pa.int64()),
                ("name", pa.string()),
                ("country", pa.string()),
                ("signup_at", pa.timestamp("us", tz="UTC")),
            ]
        ),
    )


def orders(n: int, *, customer_ids: list[int], day: datetime, seed: int = 0) -> pa.Table:
    rng = random.Random(seed)
    rows = [
        {
            "order_id": rng.randint(10**9, 10**10),
            "customer_id": rng.choice(customer_ids),
            "amount_cents": rng.randint(500, 50_000),
            "currency": "EUR",
            "status": rng.choice(["paid", "paid", "paid", "refunded"]),
            "created_at": day + timedelta(seconds=rng.randint(0, 86_400)),
        }
        for _ in range(n)
    ]
    return pa.Table.from_pylist(
        rows,
        schema=pa.schema(
            [
                ("order_id", pa.int64()),
                ("customer_id", pa.int64()),
                ("amount_cents", pa.int64()),
                ("currency", pa.string()),
                ("status", pa.string()),
                ("created_at", pa.timestamp("us", tz="UTC")),
            ]
        ),
    )
