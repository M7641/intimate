import os
import uuid
import datetime
from collections.abc import Callable
from pathlib import Path

import pyinstrument
from fastapi import Request, responses
from pure.logging import NimbusLogger

from anyio.to_thread import current_default_thread_limiter

from quartz.api.log_message import log_message

logger = NimbusLogger.get_logger(__name__)


async def profile_request(request: Request, call_next: Callable) -> responses.Response:
    if request.query_params.get("profile", None) == "1":
        logger.info("Profiling will work properly when running on async routes.")
        profiler = pyinstrument.Profiler()
        profiler.start()
        response = await call_next(request)
        profiler.stop()
        Path("profiling").mkdir(exist_ok=True)
        profiler.write_html(
            f"profiling/profiler_{request.url.path.strip('/').replace('/', '_') or 'root'}.html"
        )
        return response
    return await call_next(request)


async def add_process_time_header(
    request: Request,
    call_next: Callable,
) -> responses.Response:
    response = await call_next(request)
    response.headers["X-Request-Id"] = request.headers.get("X-Request-Id", "")
    response.headers["X-Auth-Email"] = request.headers.get("X-Auth-Email", "")
    response.headers["X-Auth-Tenant"] = request.headers.get("X-Auth-Tenant", "")
    response.headers["X-Auth-Role"] = request.headers.get("X-Auth-Role", "")
    return response


async def log_request(request: Request, call_next: Callable) -> responses.Response:
    user = request.headers.get("X-Auth-Email", "local_user")

    sesssion_id = request.cookies.get("session_id")
    if not sesssion_id:
        sesssion_id = str(uuid.uuid4())
        await log_message(
            f"Session started: {request.method} {request.url.path}",
            user,
            datetime.datetime.now().isoformat(),
        )

    response = await call_next(request)

    if not request.cookies.get("session_id"):
        response.set_cookie(
            key="session_id",
            value=sesssion_id,
            httponly=True,
            max_age=600,
        )

    return response


async def redirect_proxy_paths_middleware(
    request: Request,
    call_next: Callable,
) -> responses.Response:
    path = request.url.path

    user = os.getenv("USER")

    if path.startswith(f"/user/{user}/proxy/absolute/8050/"):
        new_path = path.replace(f"/user/{user}/proxy/absolute/8050/", "/")
        request.scope["path"] = new_path

    return await call_next(request)


async def log_threads_in_use_middleware(
    request: Request,
    call_next: Callable,
) -> responses.Response:
    response = await call_next(request)
    limiter = current_default_thread_limiter()
    logger.debug(f"Threads in use: {limiter.borrowed_tokens}/{limiter.total_tokens}")
    return response
