import logging
import sys
from pathlib import Path
from typing import Any

from database.render_templates import render_query


def _get_sql_logger() -> logging.Logger:
    """Dedicated logger for full SQL output, decoupled from NimbusFormatter.

    NimbusFormatter prepends timestamp/file:line and wraps every record in ANSI
    color codes — both ruin copy-paste of multi-line queries. Here we use a
    bare ``%(message)s`` formatter on stderr and set ``propagate=False`` so the
    root logger's handlers don't reformat our output. The hierarchical name
    lets callers redirect via ``logging.getLogger("database.sql")``.
    """
    sql_logger = logging.getLogger("database.sql")
    if not any(getattr(h, "_database_sql_default", False) for h in sql_logger.handlers):
        handler = logging.StreamHandler(sys.stderr)
        handler.setFormatter(logging.Formatter("%(message)s"))
        handler._database_sql_default = True  # type: ignore[attr-defined]
        sql_logger.addHandler(handler)
    sql_logger.setLevel(logging.INFO)
    sql_logger.propagate = False
    return sql_logger


sql_logger = _get_sql_logger()

_SQL_BANNER_TOP = "-- >>> SQL >>> ----------------------------------------------"
_SQL_BANNER_BOT = "-- <<< END <<< ----------------------------------------------"


def log_sql(query: str) -> None:
    """Emit a SQL block bracketed by banners for clean terminal copy-paste."""
    sql_logger.info("\n%s\n%s\n%s", _SQL_BANNER_TOP, query, _SQL_BANNER_BOT)


def load_sql_from_file(file_path: str | Path) -> str:
    """Load SQL query from a file."""
    sql_file = Path(file_path)
    if sql_file.exists():
        return sql_file.read_text()
    else:
        raise FileNotFoundError(f"{sql_file} does not exist")


def prepare_query(
    sql_string: str | None = None,
    sql_file: str | Path | None = None,
    params: dict[str, Any] = {},
    log_query: bool = False,
) -> str:

    if sql_file is not None:
        sql_string = load_sql_from_file(sql_file)
    else:
        sql_string = str(sql_string)

    sql_string = sql_string.strip()

    query = render_query(sql_string, params)

    if log_query:
        log_sql(query)

    return query
