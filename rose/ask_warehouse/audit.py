"""Audit-log helpers for the ask_warehouse pilot.

Every /api/ask call writes one row to ``ask_warehouse__audit`` (declared
in ``modules/welcome/db_artifacts.py``), regardless of success or
failure. The audit table is the dataset for the eval loop and the only
durable record of what the LLM was asked to do.

The DDL lives on the ``TableArtifact``; this module just owns the INSERT
shape so it stays in one place.
"""

from __future__ import annotations

import datetime
from typing import Any

from pydantic import BaseModel, ConfigDict

from common_py.io.db import AsyncDBActions

ASK_WAREHOUSE_AUDIT_TABLE = "ask_warehouse__audit"


class AuditRow(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")

    created_at: datetime.datetime
    user_email: str
    question: str
    generated_sql: str | None
    row_count: int | None
    duration_total_ms: int
    duration_llm_ms: int
    duration_sql_ms: int | None
    model_name: str
    input_tokens: int
    output_tokens: int
    error: str | None


async def write_audit_row(
    db: AsyncDBActions,
    *,
    schema: str,
    row: AuditRow,
) -> None:
    payload: dict[str, Any] = row.model_dump()
    # ``insert_data`` builds a parameterised INSERT — request-derived
    # values go through psycopg server-side binding, so the question
    # and generated_sql cannot inject through this path.
    await db.insert_data(payload, table_name=ASK_WAREHOUSE_AUDIT_TABLE, schema=schema)
