import os
from enum import Enum

import sqlalchemy


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
            **self.config
        )
        self._engine = sqlalchemy.create_engine(
            self._database_url, pool_use_lifo=True, pool_pre_ping=True
        )

    @property
    def database_url(self) -> str:
        return self._database_url

    @property
    def engine(self) -> sqlalchemy.engine.Engine:
        return self._engine


class SnowflakeConnector:

    def __init__(self) -> None:
        self.config = {
            "account": os.getenv("SNOWFLAKE_ACCOUNT"),
            "username": os.getenv("SNOWFLAKE_USERNAME"),
            "password": os.getenv("SNOWFLAKE_PASSWORD"),
            "warehouse": os.getenv("SNOWFLAKE_WAREHOUSE"),
            "database": os.getenv("SNOWFLAKE_DATABASE"),
            "schema": os.getenv("SNOWFLAKE_SCHEMA"),
        }
        # URL(**connection_kwargs)?
        self._database_url = "snowflake://{username}:{password}@{account}/{database}/{schema}".format(
            **self.config
        )
        self._engine = sqlalchemy.create_engine(
            self._database_url, pool_use_lifo=True, pool_pre_ping=True
        )

    @property
    def database_url(self) -> str:
        return self._database_url

    @property
    def engine(self) -> sqlalchemy.engine.Engine:
        return self._engine

class DataWarehouseConnector(Enum):
    Redshift = RedshiftConnector
    Snowflake = SnowflakeConnector

def get_connector() -> DataWarehouseConnector:
    db_type = os.getenv("DATA_WAREHOUSE_TYPE", "No DATA_WAREHOUSE_TYPE env variable provided")

    if db_type.lower() == "redshift":
        return DataWarehouseConnector.Redshift

    if db_type.lower() == "snowflake":
        return DataWarehouseConnector.Snowflake

    raise ValueError(db_type)
