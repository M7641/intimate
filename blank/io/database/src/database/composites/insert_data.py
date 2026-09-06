import datetime

import polars as pl
from pure.logging import NimbusLogger

from database.connection_pool import ConnectionPool2
from database.primitives.execute_query import execute_query

logger = NimbusLogger(__name__).logger


def insert_data(
    copy_of_data: pl.DataFrame,
    destination_table: str,
    schema: str = "sandpit",
    overwrite: bool = False,
    connector_name: str = "redshift",
    connection_pool: ConnectionPool2 | None = None,
) -> None:
    destination_table = destination_table.upper()

    if copy_of_data.height == 0:
        logger.warning("Provided dataFrame is empty!")
        return

    if overwrite:
        execute_query(
            sql_string="DELETE FROM {{ schema }}.{{ destination_table }}",
            params={"schema": schema, "destination_table": destination_table},
            connector_name=connector_name,
            connection_pool=connection_pool,
        )

    # Will need to think about strcuts and lists and where we deal with them.
    # I am happy to send them all to strings, but where we do it is a question.
    for col in copy_of_data.columns:
        if copy_of_data[col].dtype == pl.Struct:
            copy_of_data = copy_of_data.with_columns(
                pl.col(col).cast(pl.Utf8).alias(col)
            )
        if copy_of_data[col].dtype == pl.List:
            copy_of_data = copy_of_data.with_columns(
                pl.col(col).cast(pl.Utf8).alias(col)
            )

    columns = copy_of_data.columns
    column_names = ", ".join(columns)

    insert_template = (
        f"INSERT INTO {schema}.{destination_table} ({column_names}) VALUES "
    )

    values_clauses = []
    for row in copy_of_data.iter_rows():
        # Format each value, handling None/null values and string escaping
        formatted_values = []
        for value in row:
            if value is None:
                formatted_values.append("NULL")
            elif isinstance(value, float) and (
                value != value or value == float("inf") or value == float("-inf")
            ):
                formatted_values.append("NULL")
            elif isinstance(value, str):
                # Escape single quotes in strings
                escaped_value = value.replace("'", "''")
                formatted_values.append(f"'{escaped_value}'")
            elif isinstance(value, (datetime.date, datetime.datetime)):
                formatted_values.append(f"'{value}'")
            else:
                formatted_values.append(str(value))

        values_clause = f"({', '.join(formatted_values)})"
        values_clauses.append(values_clause)

    full_insert_query = insert_template + ", ".join(values_clauses)
    execute_query(
        sql_string=full_insert_query,
        connector_name=connector_name,
        connection_pool=connection_pool,
    )
