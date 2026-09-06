import os
from typing import Any
from pathlib import Path

from database.connectors.connector import get_connector
from database.primitives.utils import prepare_query
from database.connection_pool import ConnectionPool2

def execute_query(
    sql_string: str | None = None,
    sql_file: Path | None = None,
    params: dict[str, Any] = {},
    log_query: bool = False,
    connector_name: str = "redshift",
    connection_pool: ConnectionPool2 | None = None,
) -> None:
    """
    Executes a SQL query and commits the changes.
    """

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
        db_connection.commit()
