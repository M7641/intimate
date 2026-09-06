"""gauge — measure the exact memory + CPU footprint of a containerized web app.

Typer front-end over `runner.py`. Subcommands:
  gauge run   — build (optional), launch, drive load, report exact cgroup figures
  gauge init  — write a starter config file for an app
  gauge load  — the standalone HTTP load driver

Most invocations collapse to `gauge run --config app.toml`; the flags below stay
available as overrides on top of the file.
"""

from __future__ import annotations

import shlex
import subprocess
from pathlib import Path
from typing import Optional

import typer
from rich.console import Console
from rich.table import Table

from gauge import openapi, runner
from gauge.config import RunConfig, load_config, template
from gauge.load import drive_load

app = typer.Typer(
    add_completion=False,
    help="Measure the exact resource footprint of a containerized web app.",
)
console = Console()
err = Console(stderr=True, style="dim")

_BLOCKS = "▁▂▃▄▅▆▇█"


def _sparkline(values: list[float]) -> str:
    if not values:
        return ""
    hi = max(values) or 1.0
    return "".join(_BLOCKS[min(7, int(v / hi * 7))] for v in values)


@app.command()
def run(
    image: Optional[str] = typer.Argument(
        None, help="image to run (or config's image)"
    ),
    config: Optional[Path] = typer.Option(
        None, "--config", help="load settings from a TOML file"
    ),
    build: Optional[Path] = typer.Option(
        None, help="build context; build IMAGE from here first"
    ),
    dockerfile: Optional[Path] = typer.Option(
        None, help="Dockerfile path (with --build)"
    ),
    runtime: Optional[str] = typer.Option(
        None, help="force 'docker' or 'podman' (default: auto-detect)"
    ),
    port: Optional[int] = typer.Option(
        None, help="port the app listens on inside the container"
    ),
    host_port: Optional[int] = typer.Option(
        None, help="host port to publish (default: same as --port)"
    ),
    interval: Optional[float] = typer.Option(
        None, help="cgroup sampling period, seconds"
    ),
    ready_url: Optional[str] = typer.Option(
        None, help="poll this URL before starting load"
    ),
    ready_timeout: Optional[float] = typer.Option(None),
    load_url: Optional[str] = typer.Option(
        None, help="drive built-in load at this single URL"
    ),
    openapi_url: Optional[str] = typer.Option(
        None, help="derive load from this OpenAPI spec URL"
    ),
    requests: Optional[int] = typer.Option(
        None, "--requests", "-n", help="requests for the built-in load"
    ),
    concurrency: Optional[int] = typer.Option(
        None, "--concurrency", "-c", help="concurrent workers"
    ),
    load: Optional[str] = typer.Option(
        None, help="custom load command (escape hatch, e.g. wrk/k6)"
    ),
    max_duration: Optional[float] = typer.Option(
        None, help="hard stop after N seconds"
    ),
    memory: Optional[str] = typer.Option(
        None, help="enforce a memory cap, e.g. 400m (real cgroup limit)"
    ),
    cpus: Optional[str] = typer.Option(None, help="enforce a CPU cap, e.g. 1.5"),
    run_arg: list[str] = typer.Option(
        [], help="raw extra `run` arg (repeatable), e.g. -e KEY=val"
    ),
    out_dir: Optional[Path] = typer.Option(
        None, "--out-dir", help="write run.json + memory.png + cpu.png here"
    ),
) -> None:
    """Measure an image's exact memory + CPU via container cgroup accounting."""
    cfg = load_config(config) if config else RunConfig()
    cfg.apply_overrides(
        image=image,
        build_context=build,
        dockerfile=dockerfile,
        runtime=runtime,
        port=port,
        host_port=host_port,
        memory=memory,
        cpus=cpus,
        run_args=run_arg,
        ready_url=ready_url,
        ready_timeout=ready_timeout,
        load_url=load_url,
        openapi_url=openapi_url,
        requests=requests,
        concurrency=concurrency,
        load_command=load,
        max_duration=max_duration,
        interval=interval,
        out_dir=out_dir,
    )
    if not cfg.image:
        err.print("[red]no image given — pass one, or set `image` in the config[/red]")
        raise typer.Exit(1)
    _execute(cfg)


def _execute(cfg: RunConfig) -> None:
    try:
        rt = runner.detect_runtime(cfg.runtime)
    except RuntimeError as e:
        err.print(f"[red]{e}[/red]")
        raise typer.Exit(1)

    if cfg.build_context is not None:
        err.print(f"building [bold]{cfg.image}[/bold] from {cfg.build_context} ...")
        cmd = [
            rt,
            "build",
            "-t",
            cfg.image,
            *(["-f", str(cfg.dockerfile)] if cfg.dockerfile else []),
            str(cfg.build_context),
        ]
        if subprocess.run(cmd).returncode != 0:
            err.print("[red]build failed[/red]")
            raise typer.Exit(1)

    run_args: list[str] = []
    if cfg.memory:
        run_args.append(f"--memory={cfg.memory}")
    if cfg.cpus:
        run_args.append(f"--cpus={cfg.cpus}")
    run_args += cfg.run_args

    load_fn = _build_load_fn(cfg)

    try:
        result = runner.run(
            rt=rt,
            image=cfg.image,
            port=cfg.port,
            host_port=cfg.effective_host_port,
            interval=cfg.interval,
            ready_url=cfg.ready_url,
            ready_timeout=cfg.ready_timeout,
            load=load_fn,
            max_duration=cfg.max_duration,
            run_args=run_args,
            on_event=lambda m: err.print(m),
        )
    except (RuntimeError, subprocess.CalledProcessError) as e:
        err.print(f"[red]{e}[/red]")
        raise typer.Exit(1)

    _render(result)

    if cfg.out_dir is not None:
        from gauge import report  # lazy: only import matplotlib when actually plotting

        files = report.write_report(result, cfg.out_dir)
        err.print(
            f"report written to {cfg.out_dir}/ ({', '.join(f.name for f in files)})"
        )


def _build_load_fn(cfg: RunConfig):
    """Pick a load strategy: OpenAPI-derived, single URL, custom command, or none."""
    if cfg.openapi_url:

        def _openapi() -> None:
            spec = openapi.fetch_spec(cfg.openapi_url)
            urls = openapi.derive_get_urls(spec, cfg.base_url())
            if not urls:
                err.print(
                    "[yellow]OpenAPI spec yielded no loadable GET endpoints[/yellow]"
                )
                return
            err.print(f"OpenAPI: exercising {len(urls)} GET endpoint(s)")
            res = drive_load(urls, cfg.requests, cfg.concurrency)
            err.print(
                f"load: {res.ok}/{res.total} ok in {res.seconds:.2f}s = {res.rps:.0f} req/s across {res.n_endpoints} endpoints"
            )

        return _openapi

    if cfg.load_url:

        def _single() -> None:
            res = drive_load(cfg.load_url, cfg.requests, cfg.concurrency)
            err.print(
                f"load: {res.ok}/{res.total} ok in {res.seconds:.2f}s = {res.rps:.0f} req/s"
            )

        return _single

    if cfg.load_command:

        def _custom() -> None:
            subprocess.run(shlex.split(cfg.load_command))

        return _custom

    return None


def _render(result: runner.Run) -> None:
    s = result.summary()
    stopped = s["stopped"]
    stopped_style = "bold red" if stopped in ("OOM-killed", "exited") else "green"

    table = Table(title="Gauge", title_style="bold", show_header=False, box=None)
    table.add_column("metric", style="cyan", justify="right")
    table.add_column("value", style="white")
    table.add_row("image", str(s["image"]))
    table.add_row("runtime", str(s["runtime"]))
    table.add_row("duration", f"{s['duration_s']} s")
    table.add_row("peak memory", f"[bold]{s['exact_peak_mem_mb']} MB[/bold]")
    table.add_row("mean memory", f"{s['sampled_mean_mem_mb']} MB")
    table.add_row("memory·time", f"{s['mb_seconds']} MB·s")
    table.add_row("CPU time", f"[bold]{s['exact_cpu_seconds']} CPU·s[/bold]")
    table.add_row("peak / mean cores", f"{s['peak_cpu_cores']} / {s['mean_cpu_cores']}")
    table.add_row("stopped", f"[{stopped_style}]{stopped}[/{stopped_style}]")
    console.print(table)

    mem = [x.mem_mb for x in result.samples]
    cpu = [x.cpu_cores for x in result.samples]
    if mem:
        console.print(
            f"  [cyan]mem[/cyan] {_sparkline(mem)}  {min(mem):.0f}→{max(mem):.0f} MB"
        )
        console.print(
            f"  [cyan]cpu[/cyan] {_sparkline(cpu)}  {min(cpu):.1f}→{max(cpu):.1f} cores"
        )


@app.command()
def init(
    image: str = typer.Argument("my-image", help="image name to seed the config with"),
    port: int = typer.Option(8080, help="port the app listens on"),
    build: Optional[str] = typer.Option(
        None, help="build context to include in the config"
    ),
    openapi: bool = typer.Option(
        False, "--openapi", help="seed OpenAPI-driven load instead of a single URL"
    ),
    out: Path = typer.Option(
        Path("gauge.toml"), "--out", "-o", help="where to write the config"
    ),
) -> None:
    """Write a minimal starter config to bootstrap measuring an app."""
    if out.exists():
        err.print(f"[red]{out} already exists — choose another path with -o[/red]")
        raise typer.Exit(1)
    out.write_text(
        template(image=image, port=port, build_context=build, openapi=openapi)
    )
    console.print(
        f"wrote [bold]{out}[/bold] — edit it, then run: [cyan]gauge run --config {out}[/cyan]"
    )


@app.command()
def setup(
    runtime: str = typer.Option(
        "podman", help="runtime to bootstrap ('podman' or 'docker')"
    ),
    yes: bool = typer.Option(
        False, "--yes", "-y", help="don't prompt before installing"
    ),
) -> None:
    """Install and start the tools gauge needs (a container runtime)."""
    from gauge import setup as setup_mod

    try:
        setup_mod.ensure_runtime(
            prefer=runtime,
            assume_yes=yes,
            echo=lambda m: console.print(m),
            confirm=lambda q: yes or typer.confirm(q),
        )
    except RuntimeError as e:
        err.print(f"[red]{e}[/red]")
        raise typer.Exit(1)
    console.print("[green]ready — you can now run `gauge run`[/green]")


@app.command()
def load(
    url: str = typer.Argument(..., help="URL to hammer"),
    requests: int = typer.Option(2000, "--requests", "-n"),
    concurrency: int = typer.Option(16, "--concurrency", "-c"),
) -> None:
    """Standalone HTTP load driver."""
    res = drive_load(url, requests, concurrency)
    console.print(
        f"{res.ok}/{res.total} ok in {res.seconds:.2f}s = [bold]{res.rps:.0f} req/s[/bold]"
    )


def main() -> None:
    app()


if __name__ == "__main__":
    main()
