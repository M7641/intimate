"""Orchestration for the /api/ask pipeline.

One ``AskService.answer(question, user_email)`` call:
  1. Builds the chat messages (prompt assembly).
  2. Awaits LLM generation (deterministic single-shot).
  3. Parses the model output (SQL fence or ``-- UNSURE``).
  4. Validates SQL through ``safety.validate_and_prepare`` (read-only,
     allowlist, LIMIT injection).
  5. Executes the SQL on the warehouse pool.
  6. Writes an audit row.
  7. Returns an ``AskResponse``.

Errors at any step short-circuit to a graceful AskResponse with the
error surfaced in ``summary`` — never raises HTTP 500 for LLM /
validation / execution issues (those are *expected* failure modes
for an LLM-driven endpoint).
"""

from __future__ import annotations

import datetime
import time
from collections.abc import Awaitable, Callable
from typing import Any

import structlog
from pydantic import BaseModel, ConfigDict

from common_py.ask_warehouse.audit import AuditRow, write_audit_row
from common_py.ask_warehouse.llm import LocalLLM
from common_py.ask_warehouse.prompts import build_chat_messages
from common_py.ask_warehouse.safety import (
    ParsedAnswer,
    ParsedSQL,
    ParsedUnsure,
    UnsafeSqlError,
    parse_llm_output,
    validate_and_prepare,
)
from common_py.env import EnvManager
from common_py.io.db import AsyncDBActions

log: structlog.stdlib.BoundLogger = structlog.stdlib.get_logger("ask_warehouse.service")


class AskResponse(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")

    sql: str | None
    rows: list[dict[str, Any]]
    row_count: int
    summary: str
    duration_ms: int


class AskService:
    """Stateless orchestrator — instantiated once per app, used per request."""

    def __init__(
        self,
        *,
        llm: LocalLLM,
        db: AsyncDBActions,
        env: EnvManager,
    ) -> None:
        self._llm = llm
        self._db = db
        self._env = env

    async def answer(self, question: str, user_email: str) -> AskResponse:
        """Run the full pipeline non-streaming. Convenience wrapper that
        forwards a no-op chunk callback to ``answer_stream``."""

        async def _noop(_chunk: str) -> None:
            return None

        return await self.answer_stream(question, user_email, _noop)

    async def answer_stream(
        self,
        question: str,
        user_email: str,
        on_chunk: Callable[[str], Awaitable[None]],
        on_status: Callable[[str], Awaitable[None]] | None = None,
    ) -> AskResponse:
        """Stream the LLM output via ``on_chunk`` then run the rest of the
        pipeline (parse → validate → execute → audit) and return the final
        ``AskResponse``.

        ``on_chunk`` fires once per token-batch the model emits; the
        accumulated text is recovered from ``_GenerateResult.text`` for
        the parse step, so callers don't need to buffer themselves.
        """
        t0 = time.perf_counter()
        now = datetime.datetime.now(datetime.timezone.utc)

        # ── 1+2: LLM generation (streaming) ─────────────────────────
        messages = build_chat_messages(question)
        gen = await self._llm.astream(messages, on_chunk=on_chunk, on_status=on_status)
        log.info(
            "ask_warehouse_generated",
            user=user_email,
            input_tokens=gen.input_tokens,
            output_tokens=gen.output_tokens,
            llm_ms=gen.duration_ms,
        )

        # ── 3: parse output (SQL fence / UNSURE / plain answer) ─────
        try:
            parsed = parse_llm_output(gen.text)
        except UnsafeSqlError as e:
            return await self._finish_with_error(
                user_email=user_email,
                question=question,
                generated_sql=None,
                error=f"parse error: {e}",
                gen=gen,
                started=t0,
                sql_ms=None,
                now=now,
            )

        if isinstance(parsed, ParsedUnsure):
            return await self._finish_with_error(
                user_email=user_email,
                question=question,
                generated_sql=None,
                error=f"unsure: {parsed.reason}",
                gen=gen,
                started=t0,
                sql_ms=None,
                now=now,
            )

        if isinstance(parsed, ParsedAnswer):
            # Non-data answer — skip validate + execute and surface the
            # model's prose as the summary. Audited like any other run
            # but with no SQL / row count.
            return await self._finish_with_answer(
                user_email=user_email,
                question=question,
                answer=parsed.text,
                gen=gen,
                started=t0,
                now=now,
            )

        # ── 4: safety validation ────────────────────────────────────
        assert isinstance(parsed, ParsedSQL)
        raw_sql = parsed.sql
        try:
            safe_sql = validate_and_prepare(raw_sql)
        except UnsafeSqlError as e:
            return await self._finish_with_error(
                user_email=user_email,
                question=question,
                generated_sql=raw_sql,
                error=f"unsafe: {e}",
                gen=gen,
                started=t0,
                sql_ms=None,
                now=now,
            )

        # ── 5: execute ──────────────────────────────────────────────
        sql_t0 = time.perf_counter()
        try:
            rows = await self._db.load_data(safe_sql)
        except Exception as e:  # noqa: BLE001 — warehouse errors are expected here
            sql_ms = int((time.perf_counter() - sql_t0) * 1000)
            log.exception("ask_warehouse_sql_failed", user=user_email)
            return await self._finish_with_error(
                user_email=user_email,
                question=question,
                generated_sql=safe_sql,
                error=f"sql execution failed: {type(e).__name__}: {e}",
                gen=gen,
                started=t0,
                sql_ms=sql_ms,
                now=now,
            )
        sql_ms = int((time.perf_counter() - sql_t0) * 1000)

        # ── 6+7: audit + response ───────────────────────────────────
        total_ms = int((time.perf_counter() - t0) * 1000)
        summary = f"{len(rows)} row{'s' if len(rows) != 1 else ''} in {sql_ms} ms"
        await self._write_audit(
            AuditRow(
                created_at=now,
                user_email=user_email,
                question=question,
                generated_sql=safe_sql,
                row_count=len(rows),
                duration_total_ms=total_ms,
                duration_llm_ms=gen.duration_ms,
                duration_sql_ms=sql_ms,
                model_name=self._llm.model_name,
                input_tokens=gen.input_tokens,
                output_tokens=gen.output_tokens,
                error=None,
            )
        )
        return AskResponse(
            sql=safe_sql,
            rows=rows,
            row_count=len(rows),
            summary=summary,
            duration_ms=total_ms,
        )

    async def _finish_with_answer(
        self,
        *,
        user_email: str,
        question: str,
        answer: str,
        gen,
        started: float,
        now: datetime.datetime,
    ) -> AskResponse:
        """Success path for plain-text (non-data) answers.

        No SQL ran, no rows fetched — the model just chatted. We still
        audit the interaction so the conversational traffic is
        observable alongside data traffic, but with ``generated_sql``
        and ``row_count`` blank.
        """
        total_ms = int((time.perf_counter() - started) * 1000)
        await self._write_audit(
            AuditRow(
                created_at=now,
                user_email=user_email,
                question=question,
                generated_sql=None,
                row_count=None,
                duration_total_ms=total_ms,
                duration_llm_ms=gen.duration_ms,
                duration_sql_ms=None,
                model_name=self._llm.model_name,
                input_tokens=gen.input_tokens,
                output_tokens=gen.output_tokens,
                error=None,
            )
        )
        return AskResponse(
            sql=None,
            rows=[],
            row_count=0,
            summary=answer,
            duration_ms=total_ms,
        )

    async def _finish_with_error(
        self,
        *,
        user_email: str,
        question: str,
        generated_sql: str | None,
        error: str,
        gen,
        started: float,
        sql_ms: int | None,
        now: datetime.datetime,
    ) -> AskResponse:
        total_ms = int((time.perf_counter() - started) * 1000)
        await self._write_audit(
            AuditRow(
                created_at=now,
                user_email=user_email,
                question=question,
                generated_sql=generated_sql,
                row_count=None,
                duration_total_ms=total_ms,
                duration_llm_ms=gen.duration_ms,
                duration_sql_ms=sql_ms,
                model_name=self._llm.model_name,
                input_tokens=gen.input_tokens,
                output_tokens=gen.output_tokens,
                error=error,
            )
        )
        return AskResponse(
            sql=generated_sql,
            rows=[],
            row_count=0,
            summary=error,
            duration_ms=total_ms,
        )

    async def _write_audit(self, row: AuditRow) -> None:
        try:
            await write_audit_row(self._db, schema=self._env.return_schema, row=row)
        except Exception:  # noqa: BLE001 — never block the response on audit
            log.exception("ask_warehouse_audit_write_failed", user=row.user_email)
