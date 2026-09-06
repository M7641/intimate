import polars as pl
from database.primitives.execute_query import execute_query
from database.composites.insert_data import insert_data
from database.batching import calculate_batch_size
from pure.logging import NimbusLogger
from database.connection_pool import ConnectionPool2

logger = NimbusLogger.get_logger(__name__)

def upsert_to_db(
    schema: str,
    table_name: str,
    data_to_upsert: pl.DataFrame,
    unique_key: list[str],
    connector_name: str,
    connection_pool: ConnectionPool2 | None = None,
) -> None:
    # Check for duplicates and warn if found
    original_count = len(data_to_upsert)
    data_to_upsert = data_to_upsert.unique(subset=unique_key, keep="last")
    deduped_count = len(data_to_upsert)

    if original_count > deduped_count:
        logger.warning(
            "Found %d duplicates on key %s. Keeping last occurrence.",
            original_count - deduped_count, unique_key,
        )

    temp_table = f"{table_name}_temp"
    batch_size = calculate_batch_size(data_to_upsert)

    execute_query(
        sql_string="DELETE FROM {{ schema }}.{{ destination_table }}",
        params={"schema": schema, "destination_table": temp_table},
        connector_name=connector_name,
        connection_pool=connection_pool,
    )
    for start in range(0, data_to_upsert.height, batch_size):
        end = start + batch_size
        batch_data = data_to_upsert[start:end]
        insert_data(
            copy_of_data=batch_data,
            destination_table=temp_table,
            schema=schema,
            overwrite=False,
            connector_name=connector_name,
            connection_pool=connection_pool,
        )

    try:
        execute_query(
            sql_string="SELECT * FROM {{ schema }}.{{ table_name }} LIMIT 1;",
            params={"schema": schema, "table_name": table_name},
            connector_name=connector_name,
            connection_pool=connection_pool,
        )
        does_table_exist = True
    except Exception:
        does_table_exist = False

    if does_table_exist is False:
        execute_query(
            sql_string="""
                CREATE TABLE {{ schema }}.{{ table_name }} AS
                SELECT *, CURRENT_TIMESTAMP as updated_at
                FROM {{ schema }}.{{ temp_table }}
                WHERE 1 = 0;
            """,
            params={
                "schema": schema,
                "table_name": table_name,
                "temp_table": temp_table,
            },
            connector_name=connector_name,
            connection_pool=connection_pool,
        )

    merge_conditions = " AND ".join(
        [f"{schema}.{table_name}.{key} = source.{key}" for key in unique_key],
    )
    update_values = [i for i in data_to_upsert.columns if i not in unique_key]

    if len(update_values) > 0:
        update_query = f"""
            SET {', '.join([f'{col} = source.{col}' for col in update_values])}
            , updated_at = CURRENT_TIMESTAMP
        """
    else:
        update_query = "SET updated_at = CURRENT_TIMESTAMP"

    merge_query = f"""
        MERGE INTO {schema}.{table_name}
        USING {schema}.{temp_table} as source
        ON {merge_conditions}
        WHEN MATCHED THEN
        UPDATE {update_query}
        WHEN NOT MATCHED THEN
        INSERT ({', '.join(data_to_upsert.columns)}, updated_at)
        VALUES ({', '.join(
            [f'source.{col}' for col in data_to_upsert.columns]
        )}, CURRENT_TIMESTAMP);
    """ # noqa: S608

    execute_query(
        sql_string=merge_query,
        connector_name=connector_name,
        connection_pool=connection_pool,
    )
    execute_query(
        sql_string="DROP TABLE IF EXISTS {{ schema }}.{{ temp_table }}",
        params={"schema": schema, "temp_table": temp_table},
        connector_name=connector_name,
        connection_pool=connection_pool,
    )
