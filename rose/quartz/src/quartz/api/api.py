import os
import anyio
from contextlib import asynccontextmanager
from collections.abc import Callable, AsyncIterator
from pathlib import Path
from prometheus_fastapi_instrumentator import Instrumentator

from typing import TypedDict

from fastapi import FastAPI, Request, responses
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse
from starlette.middleware.gzip import GZipMiddleware

from quartz.api.middleware import (
    log_request,
    add_process_time_header,
    profile_request,
    redirect_proxy_paths_middleware,
    log_threads_in_use_middleware,
)

from pure.logging import NimbusLogger
from pure.env_manager import EnvManager

logger = NimbusLogger.get_logger(__name__)


class State(TypedDict):
    env_manager: EnvManager


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[State]:
    limiter = anyio.to_thread.current_default_thread_limiter()
    limiter.total_tokens = 50

    # Enable asyncio debug mode
    # will print warnings for blocking calls.
    os.environ["PYTHONASYNCIODEBUG"] = "1"
    yield {
        "env_manager": EnvManager(),
    }


app = FastAPI(lifespan=lifespan)

Instrumentator().instrument(app).expose(
    app,
    endpoint="/prometheus/metrics",
)

dist_path = Path(__file__).parents[3] / "frontend" / "dist"
dist_path.mkdir(parents=True, exist_ok=True)
index_file = dist_path / "index.html"
if not index_file.exists():
    index_file.write_text(
        "<!DOCTYPE html><html><head><title>Fake SPA</title></head><body><h1>Welcome to the Fake SPA</h1></body></html>",
        encoding="utf-8",
    )


@app.middleware("http")
async def profile_request_callback(
    request: Request,
    call_next: Callable,
) -> responses.Response:
    return await profile_request(
        request=request,
        call_next=call_next,
    )


@app.middleware("http")
async def add_process_time_header_callback(
    request: Request,
    call_next: Callable,
) -> responses.Response:
    return await add_process_time_header(
        request=request,
        call_next=call_next,
    )


@app.middleware("http")
async def log_request_callback(
    request: Request,
    call_next: Callable,
) -> responses.Response:
    return await log_request(
        request=request,
        call_next=call_next,
    )


@app.middleware("http")
async def redirect_proxy_paths(
    request: Request,
    call_next: Callable,
) -> responses.Response:
    return await redirect_proxy_paths_middleware(
        request=request,
        call_next=call_next,
    )


@app.middleware("http")
async def log_threads_in_use(
    request: Request,
    call_next: Callable,
) -> responses.Response:
    return await log_threads_in_use_middleware(
        request=request,
        call_next=call_next,
    )


@app.get("/health")
async def health() -> dict:
    return {"status": "ok"}


@app.get("/api/hello")
async def hello() -> dict:
    """A tiny example endpoint the SolidJS frontend can call."""
    return {"message": "Hello from FastAPI", "framework": "SolidJS"}


@app.get("/{full_path:path}")
async def serve_spa(full_path: str):
    """
    Catches all routes and serves the SPA index.html file.
    404s to be handled client-side.

    This is a catch all route that serves the SPA for any path not
    matched by previous routes. Therefore it should be the last route defined.
    """
    file_path = dist_path / full_path

    if full_path.startswith("api/"):
        return responses.JSONResponse(status_code=404, content={"detail": "Not Found"})

    if file_path.is_file():
        return FileResponse(file_path)
    return FileResponse(dist_path / "index.html")


app.add_middleware(GZipMiddleware, minimum_size=300, compresslevel=1)

app.mount("/", StaticFiles(directory=dist_path, html=True), name="dist")
