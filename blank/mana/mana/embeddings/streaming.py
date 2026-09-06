"""SQL streaming dataset for large-scale data that doesn't fit in memory.

Rescued from the old data/relational.py module. Paginates through a SQL table
in configurable chunks, yielding (features, target) tensor pairs.
"""

from __future__ import annotations

import logging

import polars as pl
import torch
from torch.utils.data import IterableDataset

logger = logging.getLogger(__name__)


class SQLStreamingDataset(IterableDataset):
    """Iterable PyTorch Dataset that streams rows from a SQL table in pages.

    Uses a pluggable ``load_data`` callable so it isn't coupled to any
    specific database client.  The callable receives a SQL string and a
    params dict and must return a ``polars.DataFrame``.

    Args:
        load_data: ``(sql_string, params) -> pl.DataFrame`` — database query function.
        sql_table_name: Table name.
        sql_table_schema: Schema (namespace) the table lives in.
        order_by_column: Column used for deterministic ordering.
        target_column: Name of the target/label column.
        features: List of feature column names.
        buffer_size: Number of rows fetched per page.
    """

    def __init__(
        self,
        load_data: callable,
        sql_table_name: str,
        sql_table_schema: str,
        order_by_column: str,
        target_column: str,
        features: list[str],
        buffer_size: int = 1_000,
    ):
        self.load_data = load_data
        self.sql_table_name = sql_table_name
        self.sql_table_schema = sql_table_schema
        self.order_by_column = order_by_column
        self.buffer_size = buffer_size
        self.target_column = target_column
        self.features = features

        self._validate_table()
        self._batches = self._number_of_batches()

    def _validate_table(self) -> None:
        table_df = self.load_data(
            sql_string="SELECT * FROM {{ sql_table_schema }}.{{ sql_table_name }} LIMIT 1",
            params={
                "sql_table_name": self.sql_table_name,
                "sql_table_schema": self.sql_table_schema,
            },
        )
        missing = set(self.features + [self.target_column]) - set(table_df.columns)
        if missing:
            raise ValueError(f"Missing columns in the table: {missing}")

    def _data_len(self) -> int:
        count_df = self.load_data(
            sql_string="SELECT COUNT(*) AS count FROM {{ sql_table_schema }}.{{ sql_table_name }}",
            params={
                "sql_table_name": self.sql_table_name,
                "sql_table_schema": self.sql_table_schema,
            },
        )
        row_count = count_df.item(row=0, column="count")
        logger.info(
            "Total rows in %s.%s: %d",
            self.sql_table_schema,
            self.sql_table_name,
            row_count,
        )
        return row_count

    def _number_of_batches(self) -> int:
        total_rows = self._data_len()
        return (total_rows + self.buffer_size - 1) // self.buffer_size

    def _load_page(self, batch: int) -> pl.DataFrame:
        return self.load_data(
            sql_string="""
                SELECT
                    {{ select_columns | join(", ") }}
                FROM {{ sql_table_schema }}.{{ sql_table_name }}
                ORDER BY {{ order_by_column }}
                OFFSET {{ offset }}
                LIMIT {{ limit }};
            """,
            params={
                "sql_table_name": self.sql_table_name,
                "sql_table_schema": self.sql_table_schema,
                "select_columns": self.features + [self.target_column],
                "order_by_column": self.order_by_column,
                "offset": batch * self.buffer_size,
                "limit": self.buffer_size,
            },
        )

    def __iter__(self):
        for batch_idx in range(self._batches):
            page = self._load_page(batch_idx)
            logger.info(
                "Loaded page %d/%d — %d rows (≈%.2f MB)",
                batch_idx + 1,
                self._batches,
                page.height,
                page.estimated_size() / (1024 * 1024),
            )
            for row_idx in range(page.height):
                row = page.row(index=row_idx, named=True)
                features = torch.tensor(
                    [row[f] for f in self.features], dtype=torch.float32
                )
                target = torch.tensor(
                    row[self.target_column], dtype=torch.float32
                ).unsqueeze(0)
                yield features, target
