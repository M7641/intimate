import os
from pathlib import Path
from typing import Any

import polars as pl

from database.connection_pool import ConnectionPool2
from database.connectors.connector import get_connector
from database.primitives.utils import prepare_query


def load_data_as_dicts(
    sql_string: str | None = None,
    sql_file: str | Path | None = None,
    params: dict[str, Any] = {},
    log_query: bool = False,
    connector_name: str | None = None,
    connection_pool: ConnectionPool2 | None = None,
) -> list[dict[str, Any]]:
    if not sql_string and not sql_file:
        raise ValueError("Either sql_string or sql_file must be provided.")

    query = prepare_query(
        sql_string=sql_string,
        sql_file=sql_file,
        params=params,
        log_query=log_query,
    )

    db_type = connector_name or os.getenv("DATA_WAREHOUSE_TYPE", "duckdb")

    connector = get_connector(db_type) if connection_pool is None else connection_pool
    with connector.connection() as db_connection:
        cursor = db_connection.cursor()
        cursor.execute(query)
        result = cursor.fetchall()

        if cursor.description:
            column_names = [desc[0] for desc in cursor.description]
        else:
            column_names = [f"column_{i}" for i in range(len(result[0]))]

    return [dict(zip(column_names, row)) for row in result]


def load_data_as_polars(
    sql_string: str | None = None,
    sql_file: str | Path | None = None,
    params: dict[str, Any] = {},
    data_schema: dict[str, pl.datatypes.DataType | type[pl.datatypes.DataType]]
    | None = None,
    schema_overrides: dict[str, pl.datatypes.DataType | type[pl.datatypes.DataType]]
    | None = None,
    log_query: bool = False,
    connector_name: str | None = None,
    infer_schema_length: int | None = 100,
    connection_pool: ConnectionPool2 | None = None,
) -> pl.DataFrame:
    """
    Loads data from a SQL query into a Polars DataFrame.

    Parameters:
        sql_string:
            The SQL query string or path to the SQL file.
        sql_file:
            Path to the SQL file (if different from sql_string).
        params:
            A dictionary of parameters to render the SQL query.
        log_query:
            Whether to log the executed query.
        data_schema:
            A dictionary defining the schema of the DataFrame. Will change the columns.
        schema_overrides:
            A dictionary to override specific column data types only. Only changes types.
        connector_name:
            The database connector type (e.g., 'amazon_redshift', 'snowflake', etc.).

    Returns:
        A Polars DataFrame containing the loaded data.

    Raises:
        ValueError: If neither sql_string nor sql_file is provided.
    """

    if not sql_string and not sql_file:
        raise ValueError("Either sql_string or sql_file must be provided.")

    data = load_data_as_dicts(
        sql_string=sql_string,
        sql_file=sql_file,
        params=params,
        log_query=log_query,
        connector_name=connector_name,
        connection_pool=connection_pool,
    )

    data = pl.DataFrame(
        data,
        schema=data_schema if data_schema else None,
        schema_overrides=schema_overrides if schema_overrides else None,
        infer_schema_length=infer_schema_length,
    )

    return data
