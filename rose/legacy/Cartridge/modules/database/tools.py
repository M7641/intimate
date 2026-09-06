from pathlib import Path

import polars as pl
import sqlalchemy
from database.connectors import get_connector
from jinja2 import Template


class DBObj:
    def __init__(self):
        db_ting = get_connector()
        self.db_source = db_ting.source
        self.engine = db_ting.engine

    def query(
        self,
        query: str | Path,
        params: dict = {},
        return_list: bool = False,
        schema: dict | None = None,
        print_query: bool = False,
        execute: bool = False,
        **kwargs,
    ) -> pl.DataFrame | list | None:

        if isinstance(query, Path):
            with open(query) as file:
                query = file.read()

        if params:
            template = Template(query)
            query = template.render(**params)

        if print_query:
            print(query)

        if execute:
            self.execute_query(query)
            return None

        with self.engine.connect() as conn:
            data = conn.execute(sqlalchemy.sql.text(query))

        if schema is None:
            schema = list(data.keys())

        if return_list:
            return [r[0] for r in data]

        return  pl.DataFrame(
            [list(i) for i in data],
            schema=schema,
            orient="row",
            **kwargs,
        )

    def execute_query(self, query: str) -> None:
        with self.engine.begin() as conn:
            conn.execute(sqlalchemy.sql.text(query))

    def write_to_db(
        self,
        schema: str,
        table: str,
        df: pl.DataFrame,
        overwrite: bool = False,
        **kwargs,
    ) -> None:
        schema = schema.upper()
        table = table.upper()

        df.columns = [col.upper() for col in df.columns]
        if overwrite:
            drop_table_query = f"DROP TABLE IF EXISTS {schema}.{table}"
            self.execute_query(drop_table_query)
        table_name = f"{schema}.{table}"
        for chunk in self.chunk_polars_df(df):
            chunk.write_database(
                table_name=table_name,
                connection=self.engine,
                if_table_exists="append",
                **kwargs,
            )

    @staticmethod
    def chunk_polars_df(df: pl.DataFrame) -> list[pl.DataFrame]:

        chunk_size = (200_000 // df.shape[1]) - 1
        return [
            df.slice(offset=i, length=chunk_size)
            for i in range(0, df.shape[0], chunk_size)
        ]
