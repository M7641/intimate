import os

import pandas as pd
from metis.configuration import ExplainModel
from metis.utility_tools.connect import load_dataframe_from_string_query


def return_entity_ids(
    model_config: ExplainModel,
) -> pd.DataFrame:
    schema = os.getenv("OUTPUT_DB_SCHEMA", "TRANSFORM")
    build_query = f"""
        SELECT TABLE_ID::VARCHAR AS TABLE_ID FROM {schema}.METIS_SHAP_VALUES_FOR_MODEL_EXPLAIN
        WHERE MODEL_NAME = '{model_config.name}'
    """
    output_df = load_dataframe_from_string_query(
        query_str=build_query,
        try_optimise_memory_useage=True,
    )

    return output_df["table_id"].to_list()


def return_valid_attribute_values(
    model_config: ExplainModel,
    attribute: str,
) -> pd.DataFrame:
    table = model_config.table_used_to_explain
    build_query = f"SELECT DISTINCT {attribute} FROM {table}"

    output_df = load_dataframe_from_string_query(
        query_str=build_query,
        try_optimise_memory_useage=True,
    )
    output_df.columns = output_df.columns.astype(str)

    return output_df[attribute].dropna().to_list()


def return_valid_attributes_to_filter(model_config: ExplainModel) -> pd.DataFrame:
    table_name = model_config.table_used_to_explain

    build_query = f"SELECT TOP 1 * FROM {table_name}"
    output_df = load_dataframe_from_string_query(
        query_str=build_query,
        try_optimise_memory_useage=True,
    )

    output_df.columns = output_df.columns.astype(str)

    numerics = ["int16", "int32", "int64", "float16", "float32", "float64"]
    output_df = output_df.select_dtypes(exclude=numerics)
    output_df = output_df.drop(columns=[model_config.entity_id])

    return [i for i in output_df.columns]


def return_valid_fields_to_explain_with() -> pd.DataFrame:

    schema = os.getenv("OUTPUT_DB_SCHEMA", "TRANSFORM")
    build_query = (
        f"SELECT DISTINCT FEATURE FROM {schema}.METIS_DATA_USED_TO_EVALUATE_SHAP"
    )
    output_df = load_dataframe_from_string_query(
        query_str=build_query,
        try_optimise_memory_useage=True,
    )

    output_df.columns = [i.lower() for i in output_df.columns.astype(str)]

    return output_df["feature"].dropna().to_list()
