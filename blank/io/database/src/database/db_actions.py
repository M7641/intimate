import os
from pathlib import Path
from typing import Any

import polars as pl

from database.batching import calculate_batch_size
from database.composites.create_table import create_table
from database.composites.insert_data import insert_data
from database.composites.upsert_data import upsert_to_db
from database.connection_pool import ConnectionPool2
from database.primitives.data_load import load_data_as_polars
from database.primitives.execute_query import execute_query


class DBActions:
    def __init__(
        self: "DBActions",
        connector_name: str | None = None,
        connection_pool: ConnectionPool2 | None = None,
    ) -> None:
        self.connection_pool = connection_pool

        if connector_name is None:
            self.connector_name = os.getenv("DATA_WAREHOUSE_TYPE", "")
        else:
            self.connector_name = connector_name

        if self.connector_name == "":
            raise ValueError(
                "No connector name provided and DATA_WAREHOUSE_TYPE environment variable is not set."
            )

    def close_pool(
        self: "DBActions",
    ) -> None:
        if self.connection_pool is not None:
            self.connection_pool.close()

    def load_data(
        self: "DBActions",
        sql_string: str | None = None,
        sql_file: str | Path | None = None,
        params: dict[str, Any] = {},
        log_query: bool = False,
        data_schema: dict[str, pl.datatypes.DataType | type[pl.datatypes.DataType]]
        | None = None,
        schema_overrides: dict[str, pl.datatypes.DataType | type[pl.datatypes.DataType]]
        | None = None,
        infer_schema_length: int | None = 100,
    ) -> pl.DataFrame:
        return load_data_as_polars(
            sql_string=sql_string,
            sql_file=sql_file,
            params=params,
            log_query=log_query,
            schema_overrides=schema_overrides,
            data_schema=data_schema,
            connector_name=self.connector_name,
            infer_schema_length=infer_schema_length,
            connection_pool=self.connection_pool,
        )

    def execute_query(
        self: "DBActions",
        sql_string: str | None = None,
        sql_file: Path | None = None,
        params: dict[str, Any] = {},
        log_query: bool = False,
    ) -> None:
        execute_query(
            sql_string=sql_string,
            sql_file=sql_file,
            params=params,
            log_query=log_query,
            connector_name=self.connector_name,
            connection_pool=self.connection_pool,
        )

    def write_data(
        self: "DBActions",
        data: pl.DataFrame | list[dict[str, Any]] | dict[str, Any],
        table_name: str,
        schema: str,
        overwrite: bool = False,
    ) -> None:
        if isinstance(data, dict):
            data = pl.DataFrame(data, schema=list(data.keys()))

        if isinstance(data, list):
            data = pl.DataFrame(data, schema=list(data[0].keys()))

        if overwrite:
            execute_query(
                sql_string="DELETE FROM {{ schema }}.{{ destination_table }}",
                params={"schema": schema, "destination_table": table_name},
                connector_name=self.connector_name,
                connection_pool=self.connection_pool,
            )

        if data.height == 0:
            raise ValueError("Dataframe is empty")

        batch_size = calculate_batch_size(data)
        for start in range(0, data.height, batch_size):
            end = start + batch_size
            insert_data(
                copy_of_data=data[start:end],
                schema=schema,
                destination_table=table_name,
                overwrite=False,
                connector_name=self.connector_name,
                connection_pool=self.connection_pool,
            )

    def create_table(
        self: "DBActions",
        df_schema: dict[str, pl.datatypes.DataType | type[pl.datatypes.DataType]],
        table_name: str,
        schema: str = "sandpit",
    ) -> None:
        create_table(
            copy_of_data=df_schema,
            destination_table=table_name,
            schema=schema,
            connector_name=self.connector_name,
            connection_pool=self.connection_pool,
        )

    def table_exists(
        self: "DBActions",
        table_name: str,
        schema: str,
    ) -> bool:
        try:
            execute_query(
                sql_string="SELECT * FROM {{ schema }}.{{ table_name }} LIMIT 1;",
                params={"schema": schema, "table_name": table_name},
                connector_name=self.connector_name,
                connection_pool=self.connection_pool,
            )
            return True
        except Exception:
            return False

    def upsert_data(
        self: "DBActions",
        data_to_upsert: pl.DataFrame,
        unique_key: list[str],
        table_name: str,
        schema: str,
    ) -> None:
        upsert_to_db(
            data_to_upsert=data_to_upsert,
            unique_key=unique_key,
            table_name=table_name,
            schema=schema,
            connector_name=self.connector_name,
            connection_pool=self.connection_pool,
        )
