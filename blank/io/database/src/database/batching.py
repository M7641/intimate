import math

import datetime
import polars as pl

def convert_row_to_sql_line(row: tuple) -> str:
    """
    Convert a database row (tuple) to a SQL INSERT statement line.
    """
    formatted_values = []
    for value in row:
        if value is None:
            formatted_values.append("NULL")
        elif isinstance(value, float) and (
            value != value or value == float("inf") or value == float("-inf")
        ):
            formatted_values.append("NULL")
        elif isinstance(value, str):
            escaped_value = value.replace("'", "''")
            formatted_values.append(f"'{escaped_value}'")
        elif isinstance(value, (datetime.date, datetime.datetime)):
            formatted_values.append(f"'{value}'")
        else:
            formatted_values.append(str(value))
    return f"({', '.join(formatted_values)})"


def calculate_batch_size(
    copy_of_data: pl.DataFrame,
    max_bytes: int = 15_728_640,
    tolerance: float = 1.1
) -> int:
    """
    Calculate the byte usage of a database row (tuple) when converted to a SQL INSERT statement.
    """
    # Find the row that would take the most memory when converted to SQL
    max_length = 0
    sample_row = None
    for row in copy_of_data.iter_rows():
        sql_line = convert_row_to_sql_line(row)
        row_length = len(sql_line.encode("utf-8"))
        if row_length > max_length:
            max_length = row_length
            sample_row = row

    if sample_row is None:
        raise ValueError("DataFrame is empty, cannot calculate batch size.")

    sql_row = convert_row_to_sql_line(sample_row).encode("utf-8")
    bytes_per_row = math.ceil(len(sql_row) * tolerance)

    calculated_batch_size = max(1, max_bytes // bytes_per_row)
    calculated_batch_size = min(calculated_batch_size, copy_of_data.height)
    return calculated_batch_size
