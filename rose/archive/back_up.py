"""Back up an entire Redshift schema to S3 as zstd-compressed Parquet files."""

import asyncio
import tempfile
from datetime import datetime, timezone
from pathlib import Path

import uvloop
from database import DBActions
from pure.logging import NimbusLogger
from rich.console import Console
from rich.panel import Panel
from rich.progress import (
    BarColumn,
    MofNCompleteColumn,
    Progress,
    SpinnerColumn,
    TextColumn,
    TimeElapsedColumn,
    TimeRemainingColumn,
)
from rich.table import Table
from rich.text import Text
from sthree import S3Manager

logger = NimbusLogger.get_logger(__name__)

MAX_CONCURRENCY = 4

console = Console()


def _make_progress() -> Progress:
    return Progress(
        SpinnerColumn("moon", style="cyan"),
        TextColumn("[bold orchid]{task.description}"),
        BarColumn(
            bar_width=40,
            style="bar.back",
            complete_style="magenta",
            finished_style="green",
        ),
        TextColumn("[dim]{task.fields[status]}"),
        MofNCompleteColumn(),
        TimeElapsedColumn(),
        TimeRemainingColumn(),
        console=console,
        expand=False,
    )


def _print_header(schema: str, n_tables: int, prefix: str) -> None:
    header = Text.assemble(
        ("  Schema Backup  ", "bold white on dark_magenta"),
        ("\n\n", ""),
        ("  Schema   ", "dim"),
        (schema, "bold cyan"),
        ("\n", ""),
        ("  Tables   ", "dim"),
        (str(n_tables), "bold yellow"),
        ("\n", ""),
        ("  Dest     ", "dim"),
        (f"s3://…/{'/'.join(prefix.split('/')[-3:])}", "dim italic"),
    )
    console.print()
    console.print(Panel(header, border_style="bright_magenta", padding=(1, 3)))
    console.print()


def _print_summary(
    backed_up: int, skipped: int, total_bytes: int, results_detail: list
) -> None:
    table = Table(
        border_style="bright_magenta",
        header_style="bold orchid",
        row_styles=["", "dim"],
        padding=(0, 2),
    )
    table.add_column("Table", style="cyan")
    table.add_column("Rows", justify="right", style="yellow")
    table.add_column("Cols", justify="right", style="yellow")
    table.add_column("Size", justify="right", style="magenta")
    table.add_column("", justify="center")

    for name, ok, rows, cols, size_bytes in results_detail:
        if ok:
            mb = f"{size_bytes / 1024 / 1024:.1f} MB"
            table.add_row(name, f"{rows:,}", str(cols), mb, "[green]OK")
        else:
            table.add_row(name, "—", "—", "—", "[red]SKIP")

    console.print()
    console.print(table)

    total_mb = total_bytes / 1024 / 1024
    summary = Text.assemble(
        ("\n  ", ""),
        (f"{backed_up}", "bold green"),
        (" backed up", ""),
        ("  ·  ", "dim"),
        (f"{skipped}", "bold red" if skipped else "bold green"),
        (" skipped", ""),
        ("  ·  ", "dim"),
        (f"{total_mb:.1f} MB", "bold magenta"),
        (" total", ""),
    )
    console.print(Panel(summary, border_style="bright_magenta", padding=(0, 3)))
    console.print()


async def _backup_table(
    table_name: str,
    schema: str,
    prefix: str,
    db: DBActions,
    s3: S3Manager,
    semaphore: asyncio.Semaphore,
    progress: Progress,
    overall_task_id: int,
) -> tuple[str, bool, int, int, int]:
    """Back up a single table. Returns (name, success, rows, cols, bytes_written)."""
    async with semaphore:
        task_id = progress.add_task(f"  {table_name}", total=3, status="reading…")

        try:
            df = await asyncio.to_thread(
                db.load_data,
                sql_string="SELECT * FROM {{ schema }}.{{ table_name }}",
                params={"schema": schema, "table_name": table_name},
                infer_schema_length=None,
            )
        except Exception:
            logger.exception(f"Failed to read {schema}.{table_name}")
            progress.update(task_id, completed=3, status="[red]error")
            progress.advance(overall_task_id)
            return table_name, False, 0, 0, 0

        progress.advance(task_id)

        if df.is_empty():
            progress.update(task_id, completed=3, status="[yellow]empty")
            progress.advance(overall_task_id)
            return table_name, False, 0, 0, 0

        tmp = None
        try:
            tmp = tempfile.NamedTemporaryFile(suffix=".parquet", delete=False)
            tmp_path = Path(tmp.name)
            tmp.close()

            progress.update(task_id, status="compressing…")
            await asyncio.to_thread(
                df.write_parquet, tmp_path, compression="zstd", compression_level=19
            )
            progress.advance(task_id)

            file_size = tmp_path.stat().st_size
            key = f"{prefix}/{table_name}.zstd.parquet"

            progress.update(task_id, status="uploading…")
            await asyncio.to_thread(s3.save_file, key=key, file_name=tmp_path)
            progress.advance(task_id)

            mb = f"{file_size / 1024 / 1024:.1f} MB"
            progress.update(task_id, status=f"[green]{df.height:,} rows · {mb}")
            progress.advance(overall_task_id)
            return table_name, True, df.height, df.width, file_size
        except Exception:
            logger.exception(f"Failed to back up {schema}.{table_name}")
            progress.update(task_id, completed=3, status="[red]error")
            progress.advance(overall_task_id)
            return table_name, False, 0, 0, 0
        finally:
            if tmp is not None:
                Path(tmp.name).unlink(missing_ok=True)


async def _backup_schema_async(schema: str, s3_prefix: str | None = None) -> None:
    """Async inner implementation — discovers tables, fans out backup tasks."""
    db = DBActions(connector_name="redshift")
    s3 = S3Manager()

    console.print("[dim]Discovering tables…[/]")

    tables = await asyncio.to_thread(
        db.load_data,
        sql_string=(
            "SELECT table_name "
            "FROM information_schema.tables "
            "WHERE table_schema = '{{ schema }}' "
            "AND table_type = 'BASE TABLE' "
            "ORDER BY table_name"
        ),
        params={"schema": schema},
        infer_schema_length=None,
    )

    table_names: list[str] = tables["table_name"].to_list()

    if not table_names:
        console.print("[bold yellow]No tables found — nothing to back up[/]")
        return

    today = datetime.now(tz=timezone.utc).strftime("%Y-%m-%d")
    prefix = s3_prefix or f"{s3.tenant}/datascience/backups/{schema}/{today}"

    _print_header(schema, len(table_names), prefix)

    semaphore = asyncio.Semaphore(MAX_CONCURRENCY)
    progress = _make_progress()

    with progress:
        overall_task_id = progress.add_task(
            "[bold white]Overall", total=len(table_names), status=""
        )

        results = await asyncio.gather(
            *[
                _backup_table(
                    name, schema, prefix, db, s3, semaphore, progress, overall_task_id
                )
                for name in table_names
            ]
        )

    backed_up = sum(1 for _, ok, *_ in results if ok)
    skipped = len(results) - backed_up
    total_bytes = sum(b for _, _, _, _, b in results)

    _print_summary(backed_up, skipped, total_bytes, results)


def backup_schema(schema: str, s3_prefix: str | None = None) -> None:
    """Copy every table in a Redshift schema to S3 as compressed Parquet.

    Each table is written as a single zstd-compressed Parquet file at:
        {tenant}/datascience/backups/{schema}/{YYYY-MM-DD}/{table_name}.zstd.parquet

    Up to MAX_CONCURRENCY tables are processed in parallel using uvloop,
    overlapping Redshift reads with S3 uploads.
    """
    uvloop.run(_backup_schema_async(schema, s3_prefix))
