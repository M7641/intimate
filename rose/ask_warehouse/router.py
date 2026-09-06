"""WebSocket endpoint for /api/ask.

Mounted by ``modules/welcome/backend/app.py`` (only). The
``AskService`` instance lives on ``app.state.ask_service`` — wired in the
Welcome lifespan after the LLM has finished loading.

Protocol (JSON messages over a single connection):

    client → server, once:
        {"question": "..."}

    server → client, many:
        {"type": "chunk", "text": "..."}   one per token-batch
    server → client, exactly one:
        {"type": "result", "sql": ..., "rows": [...],
         "row_count": ..., "summary": "...", "duration_ms": ...}
    server → client, on protocol errors:
        {"type": "error", "message": "..."}

After the final ``result`` (or ``error``) the server closes the socket.
"""

from typing import Annotated

import structlog
from fastapi import APIRouter, Depends, WebSocket, WebSocketDisconnect
from pydantic import BaseModel, ConfigDict, Field, ValidationError

from common_py.api.auth import CurrentUser
from common_py.ask_warehouse.service import AskService

log: structlog.stdlib.BoundLogger = structlog.stdlib.get_logger("ask_warehouse.router")

ask_router = APIRouter(prefix="/ask", tags=["Ask"])


class AskRequest(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    question: str = Field(min_length=3, max_length=1000)


def get_ask_service(websocket: WebSocket) -> AskService:
    """Pull the lifespan-loaded ``AskService`` off ``app.state``."""
    return websocket.app.state.ask_service


@ask_router.websocket("")
async def ask_ws(
    websocket: WebSocket,
    user: CurrentUser,
    service: Annotated[AskService, Depends(get_ask_service)],
) -> None:
    """Natural-language → Redshift SQL → rows, streamed token-by-token.

    Frame-by-frame ``chunk`` messages while the LLM emits its plan + SQL
    fence, then one ``result`` carrying the executed-SQL payload (or an
    error-shaped result if parsing / validation / execution fails — the
    same graceful-failure contract as the prior POST endpoint).
    """
    await websocket.accept()

    try:
        raw = await websocket.receive_json()
    except WebSocketDisconnect:
        return

    try:
        question = AskRequest(**raw).question
    except (ValidationError, TypeError) as exc:
        await websocket.send_json({"type": "error", "message": str(exc)})
        await websocket.close(code=1003)  # 1003 = unsupported data
        return

    async def on_chunk(text: str) -> None:
        await websocket.send_json({"type": "chunk", "text": text})

    async def on_status(text: str) -> None:
        await websocket.send_json({"type": "status", "text": text})

    try:
        response = await service.answer_stream(
            question, user.email, on_chunk, on_status=on_status
        )
    except Exception as exc:  # noqa: BLE001 — surface to the client, log full
        log.exception("ask_warehouse_stream_failed", user=user.email)
        try:
            await websocket.send_json(
                {
                    "type": "error",
                    "message": f"{type(exc).__name__}: {exc}",
                }
            )
        except Exception:  # noqa: BLE001 — socket may already be gone
            pass
        await websocket.close(code=1011)  # 1011 = internal server error
        return

    await websocket.send_json(
        {
            "type": "result",
            "sql": response.sql,
            "rows": response.rows,
            "row_count": response.row_count,
            "summary": response.summary,
            "duration_ms": response.duration_ms,
        }
    )
    await websocket.close()
