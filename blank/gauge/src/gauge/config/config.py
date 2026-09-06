"""Configuration: one resolved `RunConfig` that both a TOML file and CLI flags
produce, so a long command line can collapse into `gauge run --config app.toml`.

Precedence is CLI flag > config file > built-in default. The CLI passes only the
options the user actually set (everything else is `None`), and `apply_overrides`
layers those on top of whatever the file provided.
"""

from __future__ import annotations

import tomllib
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class RunConfig:
    # what to run
    image: str | None = None
    build_context: Path | None = None
    dockerfile: Path | None = None
    runtime: str | None = None
    # container shape
    port: int = 8080
    host_port: int | None = None
    memory: str | None = None
    cpus: str | None = None
    run_args: list[str] = field(default_factory=list)
    # readiness
    ready_url: str | None = None
    ready_timeout: float = 30.0
    # load — one of: a single url, an OpenAPI spec to derive urls from, a command
    load_url: str | None = None
    openapi_url: str | None = None
    requests: int = 2000
    concurrency: int = 16
    load_command: str | None = None
    # run + output
    max_duration: float | None = None
    interval: float = 0.5
    out_dir: Path | None = None

    @property
    def effective_host_port(self) -> int:
        return self.host_port or self.port

    def base_url(self) -> str:
        return f"http://127.0.0.1:{self.effective_host_port}"

    def apply_overrides(self, **overrides) -> RunConfig:
        """Set each field only when the override is meaningfully provided
        (not None, and not an empty list). Returns self for chaining."""
        for key, value in overrides.items():
            if value is None:
                continue
            if isinstance(value, list) and not value:
                continue
            setattr(self, key, value)
        return self


def load_config(path: Path) -> RunConfig:
    """Parse a nested TOML file into the flat RunConfig."""
    data = tomllib.loads(path.read_text())
    build = data.get("build", {})
    container = data.get("container", {})
    ready = data.get("ready", {})
    load = data.get("load", {})
    output = data.get("output", {})

    def _path(v):
        return Path(v) if v is not None else None

    return RunConfig(
        image=data.get("image"),
        build_context=_path(build.get("context")),
        dockerfile=_path(build.get("dockerfile")),
        runtime=data.get("runtime"),
        port=container.get("port", 8080),
        host_port=container.get("host_port"),
        memory=container.get("memory"),
        cpus=container.get("cpus"),
        run_args=list(container.get("run_args", [])),
        ready_url=ready.get("url"),
        ready_timeout=ready.get("timeout", 30.0),
        load_url=load.get("url"),
        openapi_url=load.get("openapi_url"),
        requests=load.get("requests", 2000),
        concurrency=load.get("concurrency", 16),
        load_command=load.get("command"),
        max_duration=data.get("max_duration"),
        interval=output.get("interval", 0.5),
        out_dir=_path(output.get("dir")),
    )


def template(image: str, port: int, build_context: str | None, openapi: bool) -> str:
    """A commented starter config. `gauge init` writes this to bootstrap an app."""
    build_block = (
        f'[build]\ncontext = "{build_context}"\n# dockerfile = "path/to/Dockerfile"\n\n'
        if build_context
        else '# [build]\n# context = "."          # build the image before measuring\n\n'
    )
    if openapi:
        load_block = (
            "[load]\n"
            "# Derive requests from the app's OpenAPI spec (all GET endpoints).\n"
            f'openapi_url = "http://127.0.0.1:{port}/openapi.json"\n'
            "requests = 2000\n"
            "concurrency = 16\n"
        )
    else:
        load_block = (
            "[load]\n"
            f'url = "http://127.0.0.1:{port}/"   # single endpoint to hammer\n'
            "requests = 2000\n"
            "concurrency = 16\n"
            '# openapi_url = "http://127.0.0.1:{port}/openapi.json"  # or derive from OpenAPI\n'
            '# command = "wrk -t4 -c16 -d10s http://127.0.0.1:{port}/"  # or a custom tool\n'
        ).replace("{port}", str(port))

    return f"""# gauge config — run with: gauge run --config <this file>
image = "{image}"

{build_block}[container]
port = {port}
# host_port = {port}
# memory = "512m"       # enforce a real cgroup cap; OOM is reported cleanly
# cpus = "2"
# run_args = ["-e", "KEY=value"]

[ready]
url = "http://127.0.0.1:{port}/"     # polled before load starts
timeout = 30

{load_block}
[output]
dir = "gauge-report"    # folder for run.json + memory.png + cpu.png
interval = 0.5          # cgroup sampling period, seconds
"""
