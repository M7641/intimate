"""Perf gate : compare un run pytest-benchmark à une baseline committée et échoue
si une régression dépasse le seuil.

Pourquoi un comparateur maison plutôt que `--benchmark-compare-fail` :
le stockage de pytest-benchmark est tagué par machine (`Linux-CPython-3.12-…`),
donc une baseline committée ne matche pas forcément la machine de CI et le
compare devient un *no-op silencieux*. Ici on compare deux fichiers JSON par
NOM de benchmark — machine-agnostique et explicite.

Usage:
    python bench_gate.py [baseline.json] [current.json] [seuil_%]

Réglages (env) :
    BENCH_MAX_REGRESSION  seuil de régression en %, défaut 25
    BENCH_STAT            statistique comparée, défaut "min" (la moins bruitée)
"""

import json
import os
import sys

from rich.console import Console
from rich.table import Table


def load_stats(path: str) -> dict[str, dict]:
    """name -> stats dict, depuis un JSON pytest-benchmark."""
    with open(path) as f:
        data = json.load(f)
    return {b["name"]: b["stats"] for b in data["benchmarks"]}


def fmt_seconds(value: float) -> str:
    """Formate des secondes en unité lisible (µs / ms / s)."""
    if value < 1e-3:
        return f"{value * 1e6:.1f} µs"
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
            f"[red]✗ baseline introuvable : {baseline_path}[/red]\n"
            "  Génère-la avec : [bold]moon run destiny:bench-update[/bold]"
        )
        return 1

    baseline = load_stats(baseline_path)
    current = load_stats(current_path)

    table = Table(title=f"Perf gate · stat={stat} · seuil régression {threshold:.0f}%")
    table.add_column("Benchmark", style="cyan")
    table.add_column("Baseline", justify="right")
    table.add_column("Actuel", justify="right")
    table.add_column("Δ", justify="right")
    table.add_column("Statut", justify="center")

    regressions: list[tuple[str, float]] = []
    new_benchmarks: list[str] = []

    for name in sorted(current):
        cur = current[name][stat]
        if name not in baseline:
            new_benchmarks.append(name)
            table.add_row(name, "—", fmt_seconds(cur), "—", "[yellow]NEW[/yellow]")
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

    # Benchmarks présents dans la baseline mais absents du run courant.
    dropped = sorted(set(baseline) - set(current))
    if dropped:
        console.print(f"[yellow]⚠ absents du run : {', '.join(dropped)}[/yellow]")
    if new_benchmarks:
        console.print(
            f"[yellow]⚠ nouveaux (pas dans la baseline) : "
            f"{', '.join(new_benchmarks)} — lance `moon run destiny:bench-update`[/yellow]"
        )

    if regressions:
        worst = ", ".join(f"{n} ({d:+.1f}%)" for n, d in regressions)
        console.print(
            f"[red]✗ perf gate échoué : {len(regressions)} régression(s) > "
            f"{threshold:.0f}% : {worst}[/red]"
        )
        return 1

    console.print("[green]✓ perf gate ok — aucune régression au-delà du seuil[/green]")
    return 0


if __name__ == "__main__":
    sys.exit(main())
