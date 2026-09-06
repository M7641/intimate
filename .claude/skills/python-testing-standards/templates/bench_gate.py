"""Perf gate: compare a pytest-benchmark run against a committed baseline and fail
if any regression exceeds the threshold.

Why a custom comparator instead of `--benchmark-compare-fail`:
pytest-benchmark keys its storage by machine tag (`Linux-CPython-3.12-...`), so a
committed baseline does not necessarily match the CI machine and the built-in
compare silently becomes a no-op. Here we compare two JSON files BY benchmark name
— machine-agnostic and explicit.

Usage:
    python bench_gate.py [baseline.json] [current.json] [threshold_%]

Settings (env):
    BENCH_MAX_REGRESSION  regression threshold in %, default 25
    BENCH_STAT            statistic to compare, default "min" (least noisy)
"""

import json
import os
import sys

from rich.console import Console
from rich.table import Table


def load_stats(path: str) -> dict[str, dict]:
    """name -> stats dict, from a pytest-benchmark JSON file."""
    with open(path) as f:
        data = json.load(f)
    return {b["name"]: b["stats"] for b in data["benchmarks"]}


def fmt_seconds(value: float) -> str:
    """Format seconds into a readable unit (us / ms / s)."""
    if value < 1e-3:
        return f"{value * 1e6:.1f} us"
    if value < 1.0:
        return f"{value * 1e3:.3f} ms"
    return f"{value:.3f} s"


def main() -> int:
    baseline_path = sys.argv[1] if len(sys.argv) > 1 else ".benchmarks/baseline.json"
    current_path = sys.argv[2] if len(sys.argv) > 2 else ".benchmarks/current.json"
    threshold = float(
        os.getenv("BENCH_MAX_REGRESSION", sys.argv[3] if len(sys.argv) > 3 else "25")
    )
    stat = os.getenv("BENCH_STAT", "min")

    console = Console()

    if not os.path.exists(baseline_path):
        console.print(
            f"[red]x baseline not found: {baseline_path}[/red]\n"
            "  Generate it with: [bold]moon run <project>:bench-update[/bold]"
        )
        return 1

    baseline = load_stats(baseline_path)
    current = load_stats(current_path)

    table = Table(
        title=f"Perf gate - stat={stat} - regression threshold {threshold:.0f}%"
    )
    table.add_column("Benchmark", style="cyan")
    table.add_column("Baseline", justify="right")
    table.add_column("Current", justify="right")
    table.add_column("Delta", justify="right")
    table.add_column("Status", justify="center")

    regressions: list[tuple[str, float]] = []
    new_benchmarks: list[str] = []

    for name in sorted(current):
        cur = current[name][stat]
        if name not in baseline:
            new_benchmarks.append(name)
            table.add_row(name, "-", fmt_seconds(cur), "-", "[yellow]NEW[/yellow]")
            continue

        base = baseline[name][stat]
        delta = (cur - base) / base * 100 if base else 0.0
        delta_str = f"{delta:+.1f}%"

        if delta > threshold:
            regressions.append((name, delta))
            status, delta_style = "[red]REGRESSION[/red]", "red"
        elif delta < -threshold:
            status, delta_style = "[green]FASTER[/green]", "green"
        else:
            status, delta_style = "[green]ok[/green]", "white"

        table.add_row(
            name,
            fmt_seconds(base),
            fmt_seconds(cur),
            f"[{delta_style}]{delta_str}[/{delta_style}]",
            status,
        )

    console.print(table)

    # Benchmarks present in the baseline but missing from the current run.
    dropped = sorted(set(baseline) - set(current))
    if dropped:
        console.print(f"[yellow]! missing from this run: {', '.join(dropped)}[/yellow]")
    if new_benchmarks:
        console.print(
            f"[yellow]! new (not in baseline): {', '.join(new_benchmarks)} "
            "- run `moon run <project>:bench-update`[/yellow]"
        )

    if regressions:
        worst = ", ".join(f"{n} ({d:+.1f}%)" for n, d in regressions)
        console.print(
            f"[red]x perf gate failed: {len(regressions)} regression(s) > "
            f"{threshold:.0f}%: {worst}[/red]"
        )
        return 1

    console.print("[green]ok perf gate passed - no regression beyond threshold[/green]")
    return 0


if __name__ == "__main__":
    sys.exit(main())
