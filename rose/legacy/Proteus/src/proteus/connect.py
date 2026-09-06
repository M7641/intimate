import os
from functools import lru_cache

import pandas as pd
import sqlalchemy
from nimbus.resources import tenants
from sqlalchemy.engine import Engine


class RedshiftConnector:

    def __init__(self) -> None:
        self.config = {
            "db": os.getenv("TENANT"),
            "username": os.getenv("REDSHIFT_USERNAME"),
            "password": os.getenv("REDSHIFT_PASSWORD"),
            "host": os.getenv("REDSHIFT_HOST"),
            "port": 5439,
        }
        self._database_url = "postgresql://{username}:{password}@{host}:{port}/{db}".format(
            **self.config,
        )
        self._engine = sqlalchemy.create_engine(
            self._database_url, pool_use_lifo=True, pool_pre_ping=True,
        )

    @property
    def database_url(self) -> str:
        return self._database_url

    @property
    def engine(self) -> sqlalchemy.engine.Engine:
        return self._engine


class SnowflakeConnector:

    def __init__(self) -> None:

        if os.getenv("SNOWFLAKE_AUTH_TYPE", "OTHER").lower() == "oauth":
            self.init_oauth()
        else:
            self.init_other()


    def init_oauth(self) -> None:
        client = tenants.get_client()
        credentials = client.get_credentials()
        self._database_url = credentials.get("connectionString")
        self._engine = sqlalchemy.create_engine(
            self._database_url, pool_use_lifo=True, pool_pre_ping=True,
        )

    def init_other(self) -> None:
        self.config = {
            "account": os.getenv("SNOWFLAKE_ACCOUNT"),
            "username": os.getenv("SNOWFLAKE_USERNAME"),
            "password": os.getenv("SNOWFLAKE_PASSWORD"),
            "warehouse": os.getenv("SNOWFLAKE_WAREHOUSE"),
            "database": os.getenv("SNOWFLAKE_DATABASE"),
            "schema": os.getenv("SNOWFLAKE_SCHEMA"),
        }
        self._database_url = "snowflake://{username}:{password}@{account}/{database}/{schema}".format(
            **self.config,
        )
        self._engine = sqlalchemy.create_engine(
            self._database_url, pool_use_lifo=True, pool_pre_ping=True,
        )

    @property
    def database_url(self) -> str:
        return self._database_url

    @property
    def engine(self) -> sqlalchemy.engine.Engine:
        return self._engine

def get_connector():
    db_type = os.getenv("DATA_WAREHOUSE_TYPE", "No DATA_WAREHOUSE_TYPE env variable provided")

    if db_type.lower() == "redshift":
        return RedshiftConnector

    if db_type.lower() == "snowflake":
        return SnowflakeConnector

    raise ValueError(db_type)


@lru_cache(maxsize=1, typed=True)
def create_engine(echo: bool | None = None) -> Engine:
    """Create a SQLAlchemy engine object.

    Parameters
    ----------
    echo: bool, optional
        Whether to print out all executed SQL commands

    Returns
    -------
    engine
        SQLAlchemy engine

    """
    return get_connector().engine


def load_dataframe_from_string_query(query_str: str) -> pd.DataFrame:
    """Load a pandas DataFrame from a string query.

    Args:
    ----
        query_str (str): The SQL query string.

    Returns:
    -------
        pd.DataFrame: The loaded DataFrame.
    """
    engine = create_engine()
    with engine.begin() as conn:
        result = conn.execute(query_str)
        dataFrame = pd.DataFrame(data=result.fetchall(), columns=result.keys())
    return dataFrame
