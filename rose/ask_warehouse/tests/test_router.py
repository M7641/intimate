"""WebSocket-layer tests for /api/ask.

The router was migrated from POST → WebSocket: the client opens a single
socket, sends one ``{"question": ...}`` frame, and the server streams
``chunk`` / ``status`` messages followed by exactly one ``result`` (or
``error``) before closing. These tests stub the AskService and verify the
wire protocol — never load the real LocalLLM, never hit a real warehouse.

Auth dependency-override pattern mirrors ``common_py/api/auth/test_auth.py``.
"""

from unittest.mock import AsyncMock, MagicMock

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient
from starlette.websockets import WebSocketDisconnect

from common_py.api.auth import get_auth_user
from common_py.api.auth.auth import AuthUser
from common_py.ask_warehouse.router import ask_router, get_ask_service
from common_py.ask_warehouse.service import AskResponse


@pytest.fixture()
def fake_response() -> AskResponse:
    return AskResponse(
        sql="SELECT 1 LIMIT 1",
        rows=[{"a": 1}],
        row_count=1,
        summary="1 row in 5 ms",
        duration_ms=42,
    )


@pytest.fixture()
def fake_service(fake_response: AskResponse) -> MagicMock:
    svc = MagicMock()

    # The router calls ``service.answer_stream(question, email, on_chunk,
    # on_status=...)``. We invoke ``on_chunk`` ourselves so tests can also
    # observe streamed frames travelling over the socket.
    async def _answer_stream(
        question: str, email: str, on_chunk, on_status=None
    ) -> AskResponse:
        if on_status is not None:
            await on_status("thinking")
        await on_chunk("SELECT ")
        await on_chunk("1")
        return fake_response

    svc.answer_stream = AsyncMock(side_effect=_answer_stream)
    return svc


@pytest.fixture()
def app(fake_service: MagicMock) -> FastAPI:
    """Minimal FastAPI app with just ask_router — no lifespan, no model.

    ``get_ask_service`` is overridden via ``dependency_overrides`` so we
    don't need to populate ``app.state.ask_service``. Auth is also
    overridden to skip the X-Auth-Email header dance.
    """
    test_app = FastAPI()
    test_app.include_router(ask_router, prefix="/api")
    test_app.dependency_overrides[get_ask_service] = lambda: fake_service
    test_app.dependency_overrides[get_auth_user] = lambda: AuthUser(
        email="mike@example.com"
    )
    return test_app


@pytest.fixture()
def client(app: FastAPI) -> TestClient:
    with TestClient(app) as c:
        yield c
    app.dependency_overrides.clear()


def _drain_until_terminal(ws) -> dict:
    """Read frames until a ``result`` or ``error`` envelope arrives."""
    while True:
        msg = ws.receive_json()
        if msg["type"] in ("result", "error"):
            return msg


def test_happy_ws_streams_chunks_then_result(client: TestClient) -> None:
    with client.websocket_connect("/api/ask") as ws:
        ws.send_json({"question": "any question"})
        # Drain frames; ensure we get at least one chunk + a final result.
        chunks: list[str] = []
        while True:
            msg = ws.receive_json()
            if msg["type"] == "chunk":
                chunks.append(msg["text"])
                continue
            if msg["type"] == "status":
                continue
            assert msg["type"] == "result"
            assert msg["sql"] == "SELECT 1 LIMIT 1"
            assert msg["rows"] == [{"a": 1}]
            assert msg["row_count"] == 1
            assert msg["duration_ms"] == 42
            break
        assert chunks == ["SELECT ", "1"]


@pytest.mark.parametrize(
    "payload",
    [
        {"question": "ab"},  # too short
        {"question": "x" * 1001},  # too long
        {"question": "anything", "evil": True},  # extra forbidden field
        {},  # missing question
    ],
)
def test_invalid_payload_sends_error_then_closes(
    client: TestClient, payload: dict
) -> None:
    with client.websocket_connect("/api/ask") as ws:
        ws.send_json(payload)
        msg = ws.receive_json()
        assert msg["type"] == "error"
        # Server closes the socket immediately after the error frame; the
        # next read must surface as a WebSocketDisconnect with code 1003
        # (unsupported data, per the router contract).
        with pytest.raises(WebSocketDisconnect) as exc:
            ws.receive_json()
        assert exc.value.code == 1003


def test_service_exception_surfaces_error_frame(
    app: FastAPI, fake_service: MagicMock
) -> None:
    fake_service.answer_stream = AsyncMock(side_effect=RuntimeError("boom"))
    with TestClient(app) as client:
        with client.websocket_connect("/api/ask") as ws:
            ws.send_json({"question": "any question"})
            msg = _drain_until_terminal(ws)
            assert msg["type"] == "error"
            assert "RuntimeError" in msg["message"]
            assert "boom" in msg["message"]
            with pytest.raises(WebSocketDisconnect) as exc:
                ws.receive_json()
            assert exc.value.code == 1011


# Auth wiring itself is covered by common_py/api/auth/test_auth.py — no need to
# re-test the dependency-injection mechanics here. This module tests the
# WebSocket protocol shape.
