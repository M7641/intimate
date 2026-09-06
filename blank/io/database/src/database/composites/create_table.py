import polars as pl
from pure.logging import NimbusLogger

from database.connection_pool import ConnectionPool2
from database.primitives.execute_query import execute_query

logger = NimbusLogger.get_logger(__name__)


def create_table(
    copy_of_data: pl.DataFrame,
    destination_table: str,
    schema: str = "sandpit",
    connector_name: str = "redshift",
    connection_pool: ConnectionPool2 | None = None,
) -> None:
    """
    Create a table in the database if it does not already exist.
    """

    # Check if the table exists
    try:
        execute_query(
            sql_string="SELECT * FROM {{ schema }}.{{ table_name }} LIMIT 1;",
            params={"schema": schema, "table_name": destination_table},
            connector_name=connector_name,
            connection_pool=connection_pool,
        )
    except Exception:
        return

    column_definitions = []
    for col in copy_of_data.columns:
        # Get the data type from the first non-null value in the column
        col_dtype = copy_of_data.get_column(col).dtype

        if col_dtype in [
            pl.Int8,
            pl.Int16,
            pl.Int32,
            pl.Int64,
            pl.Int128,
            pl.UInt8,
            pl.UInt16,
            pl.UInt32,
            pl.UInt64,
            pl.Boolean,
        ]:
            col_type = "INT4"
        elif col_dtype in [pl.Float64, pl.Float32, pl.Decimal]:
            col_type = "FLOAT"
        elif col_dtype in [pl.String, pl.Utf8, pl.Categorical, pl.Struct]:
            col_type = "VARCHAR"
        elif col_dtype == pl.datatypes.Date:
            col_type = "DATE"
        elif col_dtype == pl.datatypes.Datetime:
            col_type = "TIMESTAMP"
        else:
            col_type = "VARCHAR"  # Default for unknown types
            logger.warning(f"Unknown column type {col_dtype} for column {col}")

        column_definitions.append(f"{col} {col_type}")

    execute_query(
        sql_string="""
        CREATE TABLE IF NOT EXISTS {{ schema }}.{{ destination_table }}
        ({{ columns_sql }})
        """,
        params={
            "schema": schema,
            "destination_table": destination_table,
            "columns_sql": ", ".join(column_definitions),
        },
        connector_name=connector_name,
        connection_pool=connection_pool,
    )
