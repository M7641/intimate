import datetime
import os
from pathlib import Path

import polars as pl
from pure.logging import NimbusLogger
from sthree.s3 import S3Manager

from database.connection_pool import ConnectionPool2
from database.primitives.data_load import load_data_as_dicts
from database.primitives.execute_query import execute_query

logger = NimbusLogger.get_logger(__name__)


def process_data_for_copy(
    copy_of_data: pl.DataFrame,
    destination_table: str,
    schema: str,
    connector_name: str = "redshift",
    connection_pool: ConnectionPool2 | None = None,
) -> pl.DataFrame:
    """
    Prepare a DataFrame for COPY by:

        1. Stripping newlines/tabs from string columns (COPY readers choke on them).
        2. Casting columns to match the destination table types so the Parquet
           file lands with types the warehouse will happily ingest.

    Datetimes and dates stay as native Polars/Parquet types — both Redshift and
    Snowflake COPY ... PARQUET read them directly, preserving precision and
    timezone without a fragile string round-trip.
    """
    copy_of_data = copy_of_data.with_columns(
        pl.col(col).str.replace(r"[\r\n\t]", " ", literal=False)
        for col in copy_of_data.columns
        if copy_of_data[col].dtype in (pl.String, pl.Utf8)
    )

    result = load_data_as_dicts(
        sql_string="""
        SELECT column_name, data_type
        FROM information_schema.columns
        WHERE table_schema ilike '{{ schema }}'
            AND table_name ilike '{{ destination_table }}'
        """,
        params={"schema": schema, "destination_table": destination_table},
        connector_name=connector_name,
        connection_pool=connection_pool,
    )

    # Covers Redshift and Snowflake information_schema.data_type values
    # (lowercased). Microsecond is the safest Parquet precision for both.
    type_mapping: dict[str, pl.DataType] = {
        # strings
        "character varying": pl.Utf8,
        "varchar": pl.Utf8,
        "text": pl.Utf8,
        # integers
        "smallint": pl.Int16,
        "integer": pl.Int32,
        "int": pl.Int32,
        "int4": pl.Int32,
        "bigint": pl.Int64,
        "int8": pl.Int64,
        # floats / decimals
        "real": pl.Float32,
        "float4": pl.Float32,
        "double precision": pl.Float64,
        "float": pl.Float64,
        "float8": pl.Float64,
        "decimal": pl.Float64,
        "numeric": pl.Float64,
        "number": pl.Float64,  # Snowflake NUMBER(p,s)
        # booleans — Redshift historically needed Int16; keep for parity
        "boolean": pl.Int16,
        # temporal — native Parquet types, not strings
        "date": pl.Date,
        "timestamp": pl.Datetime("us"),
        "timestamp without time zone": pl.Datetime("us"),
        "timestamp with time zone": pl.Datetime("us"),
        "timestamp_ntz": pl.Datetime("us"),  # Snowflake
        "timestamp_ltz": pl.Datetime("us"),  # Snowflake
        "timestamp_tz": pl.Datetime("us"),  # Snowflake
    }

    datetime_variants = (
        pl.Datetime,
        pl.Datetime("ms"),
        pl.Datetime("us"),
        pl.Datetime("ns"),
    )

    for row in result:
        col_name = row["column_name"].lower()
        db_type = row["data_type"].lower()
        if col_name not in copy_of_data.columns or db_type not in type_mapping:
            continue
        target_type = type_mapping[db_type]
        source_dtype = copy_of_data[col_name].dtype
        # Skip re-casting already-temporal columns: a cast to naive Datetime("us")
        # would drop the timezone, which matters for TIMESTAMPTZ / TIMESTAMP_TZ.
        if target_type == pl.Date and source_dtype == pl.Date:
            continue
        if target_type in datetime_variants and source_dtype in datetime_variants:
            continue
        try:
            copy_of_data = copy_of_data.with_columns(
                pl.col(col_name).cast(target_type)
            )
        except Exception as e:
            logger.warning(
                f"Could not cast column {col_name} to {target_type}: {e}"
            )

    return copy_of_data


def copy_via_s3(
    copy_of_data: pl.DataFrame,
    destination_table: str,
    schema: str = "sandpit",
    overwrite: bool = False,
    connector_name: str = "redshift",
    connection_pool: ConnectionPool2 | None = None,
) -> None:
    """
    Copy DataFrame to S3 and use COPY command to load into database table.

    So far only tested with Redshift.
    Snowflake should be possible: https://docs.snowflake.com/en/sql-reference/sql/copy-into-table
    """

    destination_table = destination_table.upper()

    if copy_of_data.height == 0:
        logger.warning("Provided dataFrame is empty!")
        return

    if overwrite:
        execute_query(
            sql_string="DELETE FROM {{ schema }}.{{ destination_table }}",
            connector_name=connector_name,
            connection_pool=connection_pool,
            params={"schema": schema, "destination_table": destination_table},
        )

    logger.info(f"Starting copy_data_via_s3 to {destination_table}")

    s3_manager = S3Manager()
    timestamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S:%f")
    s3_key = (
        f"{s3_manager.tenant}/datascience/temp/{destination_table}/{timestamp}.parquet"
    )

    processed_data = process_data_for_copy(
        copy_of_data=copy_of_data,
        destination_table=destination_table,
        connector_name=connector_name,
        schema=schema,
        connection_pool=connection_pool,
    )

    tmp_file_name = Path.cwd() / f"{destination_table}_{timestamp}.parquet"

    processed_data.write_parquet(
        file=tmp_file_name,
        compression="zstd",
        compression_level=22,
    )
    s3_manager.save_file(key=s3_key, file_name=tmp_file_name)
    try:
        bucket_name = s3_manager.bucket_name
        s3_path = f"s3://{bucket_name}/{s3_key}"
        execute_query(
            sql_string="""
            COPY {{ schema }}.{{ destination_table }}
            FROM '{{ s3_path }}'
            CREDENTIALS 'aws_iam_role={{ redshift_iam_role }}'
            FORMAT AS PARQUET;
            """,
            params={
                "schema": schema,
                "destination_table": destination_table,
                "s3_path": s3_path,
                "redshift_iam_role": os.getenv("REDSHIFT_IAM_ROLE"),
            },
            connector_name=connector_name,
            connection_pool=connection_pool,
        )
        logger.info(f"Data copied from S3 to table {schema}.{destination_table}")
    except Exception as e:
        logger.error(f"Error copying data via S3: {e}")
        logger.error("select * from svl_s3log to diagnose S3 issues")
        raise
    finally:
        if os.path.exists(tmp_file_name):
            os.remove(tmp_file_name)
