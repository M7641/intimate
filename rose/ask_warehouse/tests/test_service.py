"""End-to-end service orchestration tests with stubbed LLM + db.

These tests don't load the real model — they validate the orchestration
flow (parse → validate → execute → audit → respond) by feeding canned
LLM outputs through the pipeline.
"""

import datetime
from unittest.mock import AsyncMock, MagicMock

import pytest

from common_py.ask_warehouse.llm import _GenerateResult
from common_py.ask_warehouse.service import AskService


def _make_service(llm_text: str, rows: list[dict] | Exception = []) -> AskService:
    llm = MagicMock()
    llm.model_name = "stub-model"
    # The service streams via ``astream`` (not ``agenerate``); AsyncMock
    # ignores extra kwargs like ``on_chunk`` / ``on_status`` so we don't
    # need to fake the callback fan-out — only the final result.
    llm.astream = AsyncMock(
        return_value=_GenerateResult(
            text=llm_text, input_tokens=100, output_tokens=50, duration_ms=120
        )
    )
    db = MagicMock()
    if isinstance(rows, Exception):
        db.load_data = AsyncMock(side_effect=rows)
    else:
        db.load_data = AsyncMock(return_value=rows)
    db.insert_data = AsyncMock(return_value=None)
    env = MagicMock()
    env.return_schema = "test_schema"
    return AskService(llm=llm, db=db, env=env)


@pytest.mark.asyncio
async def test_happy_path_returns_rows_and_audits() -> None:
    service = _make_service(
        llm_text="```sql\nSELECT date_week FROM sales_product_week\n```",
        rows=[{"date_week": datetime.date(2026, 5, 11)}],
    )
    response = await service.answer("recent sales weeks", "mike@example.com")

    assert response.row_count == 1
    assert response.rows == [{"date_week": datetime.date(2026, 5, 11)}]
    assert response.sql is not None
    assert "LIMIT" in response.sql  # safety injected it
    assert response.summary.startswith("1 row")

    # Audit was written exactly once (success path).
    service._db.insert_data.assert_called_once()


@pytest.mark.asyncio
async def test_unsure_short_circuits_without_executing_sql() -> None:
    service = _make_service(
        llm_text="-- UNSURE: this question requires planogram data which is not in the curated schema",
    )
    response = await service.answer("show me planograms", "mike@example.com")

    assert response.sql is None
    assert response.rows == []
    assert "unsure" in response.summary.lower()
    # SQL was never executed.
    service._db.load_data.assert_not_called()
    # Audit was still written.
    service._db.insert_data.assert_called_once()


@pytest.mark.asyncio
async def test_unsafe_sql_blocked_before_execution() -> None:
    service = _make_service(
        llm_text="```sql\nDELETE FROM sales_product_week\n```",
    )
    response = await service.answer("delete some sales", "mike@example.com")

    assert "unsafe" in response.summary.lower()
    service._db.load_data.assert_not_called()
    service._db.insert_data.assert_called_once()  # audit row still written


@pytest.mark.asyncio
async def test_sql_execution_failure_surfaces_in_summary() -> None:
    service = _make_service(
        llm_text="```sql\nSELECT * FROM sales_product_week\n```",
        rows=RuntimeError("redshift timeout"),
    )
    response = await service.answer("anything", "mike@example.com")

    assert "sql execution failed" in response.summary.lower()
    assert "redshift timeout" in response.summary
    service._db.insert_data.assert_called_once()


@pytest.mark.asyncio
async def test_audit_failure_does_not_break_response() -> None:
    service = _make_service(
        llm_text="```sql\nSELECT 1\n```",
        rows=[{"col": 1}],
    )
    service._db.insert_data = AsyncMock(side_effect=RuntimeError("audit DB down"))

    response = await service.answer("anything", "mike@example.com")
    # User still gets their answer even if audit write failed.
    assert response.row_count == 1
