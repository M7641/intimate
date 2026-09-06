"""Typer CLI — saga's API on the command line.

saga demo                                  # end-to-end narrative (throwaway DB)
saga remember --session monday "I'm based in Manchester"   # ingest a message
saga recall "where do I live?"             # hybrid retrieval
saga context --session monday "what do I like?"   # block ready to inject
saga inspect                               # dump facts (including stale ones)
"""

from __future__ import annotations

from pathlib import Path
from typing import Annotated

import typer
from rich.console import Console
from rich.table import Table

from saga.memory import Memory
from saga.store import DEFAULT_DB

app = typer.Typer(
    name="saga",
    help="Context database for AI agents — the mechanical core, offline.",
    no_args_is_help=True,
    add_completion=False,
)
console = Console()

DbOpt = Annotated[Path, typer.Option(help="SQLite memory file.")]
SessionOpt = Annotated[str, typer.Option(help="Session/agent identifier.")]


@app.command()
def demo() -> None:
    """Run the narrative demo (temporary throwaway DB)."""
    from saga.demo import main as demo_main

    raise typer.Exit(demo_main())


@app.command()
def remember(
    text: Annotated[str, typer.Argument(help="The message to ingest.")],
    session: SessionOpt = "default",
    role: Annotated[str, typer.Option(help="user | agent.")] = "user",
    db: DbOpt = DEFAULT_DB,
) -> None:
    """Ingest a message: store the episode + consolidate facts."""
    mem = Memory(db)
    w = mem.remember(session, role, text)
    for f in w.added:
        console.print(f"[green]+ fact[/]  {f}")
    for f in w.invalidated:
        console.print(f"[red]~ closed[/]  {f}")
    if not w.added:
        console.print("[dim](no durable fact extracted from this message)[/]")
    mem.close()


@app.command()
def recall(
    query: Annotated[str, typer.Argument(help="The retrieval query.")],
    k: Annotated[int, typer.Option(help="Number of facts to return.")] = 5,
    asof: Annotated[int, typer.Option(help="Time-travel: a past tick.")] = -1,
    db: DbOpt = DEFAULT_DB,
) -> None:
    """Hybrid retrieval (vector + keyword + recency)."""
    mem = Memory(db)
    results = mem.recall(query, k=k, asof=None if asof < 0 else asof)
    if not results:
        console.print("[dim](no relevant fact)[/]")
        raise typer.Exit()
    table = Table("score", "fact", "why")
    for s in results:
        table.add_row(f"{s.score:.4f}", s.fact.text, f"[dim]{s.why}[/]")
    console.print(table)
    mem.close()


@app.command()
def context(
    query: Annotated[str, typer.Argument(help="The current turn's query.")],
    session: SessionOpt = "default",
    db: DbOpt = DEFAULT_DB,
) -> None:
    """Print the <memory> block we would paste into the system prompt."""
    mem = Memory(db)
    console.print(mem.assemble_context(session, query))
    mem.close()


@app.command()
def inspect(db: DbOpt = DEFAULT_DB) -> None:
    """Dump every fact, current AND stale (shows the bi-temporality)."""
    mem = Memory(db)
    table = Table("id", "key", "object", "valid_from", "valid_to", "state")
    for f in mem.store.all_facts():
        state = "[green]current[/]" if f.valid_to is None else "[dim]stale[/]"
        table.add_row(
            str(f.id),
            f.key,
            f.object,
            str(f.valid_from),
            "—" if f.valid_to is None else str(f.valid_to),
            state,
        )
    console.print(table)
    mem.close()


def main() -> None:
    app()


if __name__ == "__main__":
    main()
