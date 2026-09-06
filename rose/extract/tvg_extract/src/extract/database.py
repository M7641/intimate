"""Minimal Snowflake data access.

A single, dependency-light module exposing three things:

* :func:`render_query`     — Jinja2 templating for SQL strings.
* :class:`SnowflakeConnector` — a thin Snowflake connection wrapper.
* :class:`DBActions`       — high-level helpers (load / execute / write / upsert).

This used to be a multi-backend abstraction (Redshift, DuckDB, MSSQL, libsql)
with connection pooling and S3 COPY. It has been collapsed to Snowflake only.
"""

from __future__ import annotations

import datetime
import json
import logging
import math
import os
import sys
from typing import Any

import polars as pl
import requests
import snowflake.connector
from jinja2 import Template

__all__ = ["DBActions", "SnowflakeConnector", "render_query"]

logger = logging.getLogger("database")


def render_query(sql_string: str, parameters: dict[str, Any] | None = None) -> str:
    """Render a parameterised SQL query with Jinja2."""
    return Template(sql_string.strip()).render(**(parameters or {}))


def get_sql_logger() -> logging.Logger:
    """Dedicated logger for full SQL output with a bare ``%(message)s`` format.

    ``propagate=False`` stops the root logger from reformatting multi-line
    queries, keeping them clean to copy-paste from the terminal.
    """
    sql_logger = logging.getLogger("database.sql")
    if not any(getattr(h, "database_sql_default", False) for h in sql_logger.handlers):
        handler = logging.StreamHandler(sys.stderr)
        handler.setFormatter(logging.Formatter("%(message)s"))
        handler.database_sql_default = True  # type: ignore[attr-defined]
        sql_logger.addHandler(handler)
    sql_logger.setLevel(logging.INFO)
    sql_logger.propagate = False
    return sql_logger


sql_logger = get_sql_logger()
SQL_BANNER_TOP = "-- >>> SQL >>> ----------------------------------------------"
SQL_BANNER_BOT = "-- <<< END <<< ----------------------------------------------"


def log_sql(query: str) -> None:
    sql_logger.info("\n%s\n%s\n%s", SQL_BANNER_TOP, query, SQL_BANNER_BOT)


def prepare_query(
    query: str,
    params: dict[str, Any] | None = None,
    log_query: bool = False,
) -> str:
    """Render a SQL query with optional Jinja2 parameters and logging."""
    rendered = render_query(query, params or {})
    if log_query:
        log_sql(rendered)
    return rendered


# --------------------------------------------------------------------------- #
# Batching helpers (used by write / upsert)                                   #
# --------------------------------------------------------------------------- #
def row_to_sql(row: tuple) -> str:
    """Format a row tuple as a SQL ``VALUES`` clause, escaping as needed."""
    formatted: list[str] = []
    for value in row:
        if value is None:
            formatted.append("NULL")
        elif isinstance(value, float) and (
            value != value or value in (float("inf"), float("-inf"))
        ):
            formatted.append("NULL")
        elif isinstance(value, str):
            formatted.append("'" + value.replace("'", "''") + "'")
        elif isinstance(value, (datetime.date, datetime.datetime)):
            formatted.append(f"'{value}'")
        else:
            formatted.append(str(value))
    return f"({', '.join(formatted)})"


def calculate_batch_size(
    data: pl.DataFrame, max_bytes: int = 15_728_640, tolerance: float = 1.1
) -> int:
    """Rows per INSERT batch, sized from the heaviest row to stay under ``max_bytes``."""
    max_length = 0
    sample_row: tuple | None = None
    for row in data.iter_rows():
        row_length = len(row_to_sql(row).encode("utf-8"))
        if row_length > max_length:
            max_length, sample_row = row_length, row
    if sample_row is None:
        raise ValueError("DataFrame is empty, cannot calculate batch size.")
    bytes_per_row = math.ceil(len(row_to_sql(sample_row).encode("utf-8")) * tolerance)
    return min(max(1, max_bytes // bytes_per_row), data.height)


def snowflake_type(dtype: Any) -> str:
    """Map a Polars dtype to a Snowflake column type."""
    if dtype in (
        pl.Int8,
        pl.Int16,
        pl.Int32,
        pl.Int64,
        pl.Int128,
        pl.UInt8,
        pl.UInt16,
        pl.UInt32,
        pl.UInt64,
    ):
        return "NUMBER"
    if dtype == pl.Boolean:
        return "BOOLEAN"
    if dtype in (pl.Float32, pl.Float64, pl.Decimal):
        return "FLOAT"
    if dtype in (pl.String, pl.Utf8, pl.Categorical, pl.Struct):
        return "VARCHAR"
    if dtype == pl.Date:
        return "DATE"
    if dtype == pl.Datetime:
        return "TIMESTAMP_NTZ"
    logger.warning("Unknown column type %s; defaulting to VARCHAR", dtype)
    return "VARCHAR"


# --------------------------------------------------------------------------- #
# Connector                                                                   #
# --------------------------------------------------------------------------- #
class SnowflakeConnector:
    """Thin wrapper around ``snowflake.connector``.

    Auth is OAuth when ``SNOWFLAKE_AUTH_TYPE=oauth`` (fetches a Nimbus token),
    otherwise username/password read from ``SNOWFLAKE_*`` environment variables.
    """

    def __init__(self) -> None:
        self.con = self._connect()

    def _connect(self) -> Any:
        """Open a connection using whichever auth the environment selects."""
        if os.getenv("SNOWFLAKE_AUTH_TYPE", "OTHER").lower() == "oauth":
            return self.connect_oauth()
        return self.connect_password()

    def reconnect(self) -> Any:
        """Drop the current connection and open a fresh one.

        For OAuth this fetches a new Nimbus access token, recovering from an
        expired-token error without restarting the process.
        """
        self.close()
        self.con = self._connect()
        return self.con

    @staticmethod
    def connect_oauth() -> Any:

        api_result = requests.get(
            "https://service.nimbus.example/connections/api/v1/connections/credentials",
            headers={"Authorization": os.getenv("API_KEY")},
            timeout=10,
        )

        if api_result.status_code != 200:
            raise ValueError(
                f"Failed to retrieve credentials: HTTP {api_result.status_code}"
            )

        credentials: dict[str, str] = json.loads(api_result.content.decode())

        return snowflake.connector.connect(
            user=credentials.get("user", ""),
            account=credentials.get("host", ""),
            database=credentials.get("database", ""),
            schema=credentials.get("schema", ""),
            warehouse=credentials.get("warehouse", ""),
            authenticator="oauth",
            token=credentials.get("accessToken", ""),
        )

    @staticmethod
    def connect_password() -> Any:
        return snowflake.connector.connect(
            user=os.getenv("SNOWFLAKE_USERNAME"),
            password=os.getenv("SNOWFLAKE_PASSWORD"),
            account=os.getenv("SNOWFLAKE_ACCOUNT"),
            warehouse=os.getenv("SNOWFLAKE_WAREHOUSE"),
            database=os.getenv("SNOWFLAKE_DATABASE"),
            schema=os.getenv("SNOWFLAKE_SCHEMA"),
        )

    def connection(self) -> Any:
        return self.con

    def close(self) -> None:
        if getattr(self, "con", None) is not None:
            self.con.close()

    def __enter__(self) -> "SnowflakeConnector":
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()


# --------------------------------------------------------------------------- #
# High-level actions                                                          #
# --------------------------------------------------------------------------- #
# Snowflake error number for "Authentication token has expired" (SQLState 08001).
TOKEN_EXPIRED_ERRNO = 390114


def is_token_expired(exc: Exception) -> bool:
    """True if ``exc`` is Snowflake reporting an expired auth token."""
    if getattr(exc, "errno", None) == TOKEN_EXPIRED_ERRNO:
        return True
    message = str(getattr(exc, "msg", "") or exc)
    return "authentication token has expired" in message.lower()


class DBActions:
    """High-level Snowflake operations backed by a single reused connection."""

    def __init__(self, connector: SnowflakeConnector | None = None) -> None:
        self.connector = connector or SnowflakeConnector()

    @property
    def con(self) -> Any:
        return self.connector.connection()

    def close(self) -> None:
        """Close the underlying Snowflake connection."""
        self.connector.close()

    def _run(self, operation: Any, *, retry: bool = True) -> Any:
        """Run ``operation(cursor)`` against a fresh cursor.

        If Snowflake reports an expired auth token, reconnect once (fetching a
        new OAuth token) and replay the operation. Any other error propagates.
        """
        try:
            with self.con.cursor() as cursor:
                return operation(cursor)
        except snowflake.connector.errors.Error as exc:
            if retry and is_token_expired(exc):
                logger.warning(
                    "Snowflake auth token expired; reconnecting and retrying."
                )
                self.connector.reconnect()
                return self._run(operation, retry=False)
            raise

    def execute_query(
        self,
        query: str,
        params: dict[str, Any] | None = None,
        log_query: bool = False,
    ) -> None:
        """Render and execute a statement, committing on success."""
        rendered = prepare_query(query, params, log_query)

        def operation(cursor: Any) -> None:
            cursor.execute(rendered)
            self.con.commit()

        self._run(operation)

    def fetch_dicts(self, query: str) -> list[dict[str, Any]]:
        def operation(cursor: Any) -> list[dict[str, Any]]:
            cursor.execute(query)
            rows = cursor.fetchall()
            if cursor.description:
                columns = [desc[0] for desc in cursor.description]
            else:
                columns = [f"column_{i}" for i in range(len(rows[0]))] if rows else []
            return [dict(zip(columns, row)) for row in rows]

        return self._run(operation)

    def load_data(
        self,
        query: str,
        params: dict[str, Any] | None = None,
        log_query: bool = False,
        data_schema: dict[str, Any] | None = None,
        schema_overrides: dict[str, Any] | None = None,
        infer_schema_length: int | None = 100,
    ) -> pl.DataFrame:
        """Run a query and return the result as a Polars DataFrame."""
        rendered = prepare_query(query, params, log_query)
        rows = self.fetch_dicts(rendered)
        return pl.DataFrame(
            rows,
            schema=data_schema or None,
            schema_overrides=schema_overrides or None,
            infer_schema_length=infer_schema_length,
        )

    # --- writes ----------------------------------------------------------- #
    def insert(self, data: pl.DataFrame, table_name: str, schema: str) -> None:
        for col in data.columns:
            if data[col].dtype in (pl.Struct, pl.List):
                data = data.with_columns(pl.col(col).cast(pl.Utf8))
        columns = ", ".join(data.columns)
        values = ", ".join(row_to_sql(row) for row in data.iter_rows())
        self.execute_query(
            f"INSERT INTO {schema}.{table_name} ({columns}) VALUES {values}"  # noqa: S608
        )

    def write_data(
        self,
        data: pl.DataFrame | list[dict[str, Any]] | dict[str, Any],
        table_name: str,
        schema: str,
        overwrite: bool = False,
    ) -> None:
        """Insert a DataFrame (or dict/list of dicts) in size-bounded batches."""
        if isinstance(data, dict):
            data = pl.DataFrame(data, schema=list(data.keys()))
        elif isinstance(data, list):
            data = pl.DataFrame(data, schema=list(data[0].keys()))

        if data.height == 0:
            raise ValueError("Dataframe is empty")

        if overwrite:
            self.execute_query(
                "DELETE FROM {{ schema }}.{{ table }}",
                params={"schema": schema, "table": table_name},
            )

        batch_size = calculate_batch_size(data)
        for start in range(0, data.height, batch_size):
            self.insert(data[start : start + batch_size], table_name, schema)

    def create_table(
        self,
        df_schema: dict[str, Any],
        table_name: str,
        schema: str = "sandpit",
    ) -> None:
        """Create a table from a ``{column: polars_dtype}`` mapping if absent."""
        if self.table_exists(table_name, schema):
            return
        columns_sql = ", ".join(
            f"{col} {snowflake_type(dtype)}" for col, dtype in df_schema.items()
        )
        self.execute_query(
            "CREATE TABLE IF NOT EXISTS {{ schema }}.{{ table }} ({{ columns }})",
            params={"schema": schema, "table": table_name, "columns": columns_sql},
        )

    def table_exists(self, table_name: str, schema: str) -> bool:
        try:
            self.execute_query(
                "SELECT * FROM {{ schema }}.{{ table }} LIMIT 1;",
                params={"schema": schema, "table": table_name},
            )
            return True
        except Exception:
            return False

    def upsert_data(
        self,
        data_to_upsert: pl.DataFrame,
        unique_key: list[str],
        table_name: str,
        schema: str,
    ) -> None:
        """Upsert via a temp table and a Snowflake ``MERGE`` on ``unique_key``."""
        original_count = len(data_to_upsert)
        data_to_upsert = data_to_upsert.unique(subset=unique_key, keep="last")
        if original_count > len(data_to_upsert):
            logger.warning(
                "Found %d duplicates on key %s. Keeping last occurrence.",
                original_count - len(data_to_upsert),
                unique_key,
            )

        # Create the target table (with audit column) if it does not exist yet.
        if not self.table_exists(table_name, schema):
            columns_sql = ", ".join(
                f"{col} {snowflake_type(dtype)}"
                for col, dtype in data_to_upsert.schema.items()
            )
            self.execute_query(
                "CREATE TABLE {{ schema }}.{{ table }} "
                "({{ columns }}, updated_at TIMESTAMP_NTZ)",
                params={"schema": schema, "table": table_name, "columns": columns_sql},
            )

        # Empty temp clone of the target structure — session-scoped, auto-dropped.
        temp_table = f"{table_name}_temp"
        self.execute_query(
            "CREATE OR REPLACE TEMPORARY TABLE {{ schema }}.{{ temp }} AS "
            "SELECT * FROM {{ schema }}.{{ table }} WHERE 1 = 0",
            params={"schema": schema, "temp": temp_table, "table": table_name},
        )

        batch_size = calculate_batch_size(data_to_upsert)
        for start in range(0, data_to_upsert.height, batch_size):
            self.insert(data_to_upsert[start : start + batch_size], temp_table, schema)

        on_clause = " AND ".join(f"t.{key} = s.{key}" for key in unique_key)
        update_cols = [c for c in data_to_upsert.columns if c not in unique_key]
        set_clause = ", ".join(f"{col} = s.{col}" for col in update_cols)
        set_clause = (
            f"{set_clause}, " if set_clause else ""
        ) + "updated_at = CURRENT_TIMESTAMP"
        insert_cols = ", ".join(data_to_upsert.columns)
        insert_vals = ", ".join(f"s.{col}" for col in data_to_upsert.columns)

        self.execute_query(
            f"""
            MERGE INTO {schema}.{table_name} t
            USING {schema}.{temp_table} s ON {on_clause}
            WHEN MATCHED THEN UPDATE SET {set_clause}
            WHEN NOT MATCHED THEN INSERT ({insert_cols}, updated_at)
            VALUES ({insert_vals}, CURRENT_TIMESTAMP)
            """  # noqa: S608
        )
        self.execute_query(
            "DROP TABLE IF EXISTS {{ schema }}.{{ temp }}",
            params={"schema": schema, "temp": temp_table},
        )
