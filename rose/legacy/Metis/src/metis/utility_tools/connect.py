"""Module for connecting to and working with the database."""

import logging
import os
from functools import lru_cache

import pandas as pd
import sqlalchemy
from nimbus.resources import tenants
from sqlalchemy.engine import Engine
from sqlalchemy.exc import ProgrammingError


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


def infer_int_columns(data_frame: pd.DataFrame) -> list:
    df_types = (
        pd.DataFrame(data_frame.apply(pd.api.types.infer_dtype, axis=0))
        .reset_index()
        .rename(columns={"index": "column", 0: "type"})
    )
    numeric_columns = df_types.query(
        "type in ['decimal', 'integer', 'floating', 'mixed-integer-float', 'mixed-integer']",
    )["column"].to_list()
    return numeric_columns


def infer_float_columns(data_frame: pd.DataFrame) -> list:
    df_types = (
        pd.DataFrame(data_frame.apply(pd.api.types.infer_dtype, axis=0))
        .reset_index()
        .rename(columns={"index": "column", 0: "type"})
    )
    numeric_columns = df_types.query(
        "type in ['decimal', 'integer', 'floating', 'mixed-integer-float', 'mixed-integer']",
    )["column"].to_list()
    return numeric_columns


def infer_categorical_columns(data_frame: pd.DataFrame) -> list:
    df_types = (
        pd.DataFrame(data_frame.apply(pd.api.types.infer_dtype, axis=0))
        .reset_index()
        .rename(columns={"index": "column", 0: "type"})
    )
    categorical_columns = df_types.query(
        "type in ['string', 'unicode', 'boolean', 'categorical']",
    )["column"].to_list()

    return categorical_columns


def optimize_memory_usage(data_frame: pd.DataFrame) -> pd.DataFrame:

    int_columns = infer_int_columns(data_frame)
    float_columns = infer_float_columns(data_frame)
    categorical_columns = infer_categorical_columns(data_frame)

    for column in int_columns:
        if data_frame[column].min() >= 0:
            data_frame[column] = pd.to_numeric(
                data_frame[column],
                downcast="unsigned",
            )
        else:
            data_frame[column] = pd.to_numeric(
                data_frame[column],
                downcast="integer",
            )

    for column in float_columns:
        data_frame[column] = pd.to_numeric(data_frame[column], downcast="float")

    for column in categorical_columns:
        data_frame[column] = data_frame[column].astype("category")

    return data_frame


def load_dataframe_from_string_query(
    query_str: str, try_optimise_memory_useage: bool = False
) -> pd.DataFrame:
    """
    Load a pandas DataFrame from a string query.

    Args:
        query_str (str): The SQL query string.

    Returns:
        pd.DataFrame: The loaded DataFrame.
    """
    engine = create_engine()
    with engine.begin() as conn:
        result = conn.execute(query_str)
        data_frame = pd.DataFrame(data=result.fetchall(), columns=result.keys())

    data_frame.columns = map(str, data_frame.columns)

    if try_optimise_memory_useage:
        try:
            data_frame = optimize_memory_usage(data_frame)
        except Exception as e:
            logging.exception(e)

    return data_frame


def upload_data_to_table(schema: str, table: str, data: pd.DataFrame) -> None:
    # will need chunking for other databases
    data.to_sql(
        table,
        get_connector().engine,
        schema=schema,
        if_exists="append",
        index=False,
        method="multi",
    )


def drop_model_data_from_table(schema: str, table: str, model_name: str) -> None:
    """
    Cleans the specified database tables for a given model.

    Args:
        schema (str): The name of the database schema.
        model_name (str): The name of the model.

    Returns:
        None
    """
    build_query = f"DELETE FROM {schema}.{table} WHERE MODEL_NAME = '{model_name}'"
    engine = create_engine()
    with engine.connect() as conn:
        try:
            conn.execute(build_query)
        except ProgrammingError:
            msg = f"No data to delete in {schema}.{table} for the model '{model_name}'."
            logging.exception(msg)


def drop_table(schema: str, table: str) -> None:
    """
    Drops the specified table if it exists.

    Args:
        schema (str): The name of the database schema.
        table (str): The name of the table.

    Returns:
        None
    """
    build_query = f"DROP TABLE {schema}.{table}"
    engine = create_engine()
    with engine.connect() as conn:
        conn.execute(build_query)
