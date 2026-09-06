import os
from database.types import Connector

def get_connector(db_type: str | None = None) -> Connector:
    """
    Get the appropriate data warehouse connector based on the db_type.

    Only import if the db_type is requested to avoid unnecessary dependencies.
    """

    if db_type is None:
        db_type = os.getenv("DATA_WAREHOUSE_TYPE")

    if db_type == "duckdb":
        from database.connectors.duckdb import DuckDBConnector
        connector = DuckDBConnector
    elif db_type == "mssql":
        from database.connectors.mssql import MSSQLConnector
        connector = MSSQLConnector
    elif db_type == "redshift" or db_type == "amazon_redshift":
        from database.connectors.redshift import RedshiftConnector
        connector = RedshiftConnector
    elif db_type == "snowflake":
        from database.connectors.snowflake import SnowflakeConnector
        connector = SnowflakeConnector
    elif db_type == "libsql":
        from database.connectors.libsql import LibsqlConnector
        connector = LibsqlConnector
    else:
        raise ValueError(f"Unsupported db_type: {db_type}")

    if connector:
        return connector()

    raise ValueError(db_type)
