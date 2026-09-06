import pandas as pd
from metis.configuration import ExplainModel
from metis.utility_tools.connect import load_dataframe_from_string_query


def return_data_for_explain(config: ExplainModel) -> pd.DataFrame:
    """
    Generates a DataFrame containing data for explaining a given configuration.

    Args:
        config (ExplainModel): The configuration settings.

    Returns:
        pd.DataFrame: A DataFrame containing the data for explaining the response variable.
    """

    if config.fields_to_explain_with == "*":
        select_group = "*"
    else:
        fields_to_explain_with = config.fields_to_explain_with
        select_group = ", ".join(fields_to_explain_with)

    build_query = f"""
        SELECT {config.entity_id} AS TABLE_ID, {select_group}
        FROM {config.table_used_to_explain}
        WHERE {config.entity_id} IN (
            SELECT {config.entity_id} FROM {config.response_variable_table_name}
        )
    """
    return load_dataframe_from_string_query(build_query)


def return_model_target_for_explain(config: ExplainModel) -> pd.DataFrame:
    """
    Returns the model target for explanation.

    Args:
        config (ExplainModel): The configuration settings.

    Returns:
        pd.DataFrame: The model target for explanation.
    """
    build_query = f"""
        SELECT {config.entity_id} AS TABLE_ID, {config.response_variable_field_name} as target
        FROM  {config.response_variable_table_name}
        WHERE {config.entity_id} IN (
            SELECT {config.entity_id} FROM {config.table_used_to_explain}
        )
    """
    return load_dataframe_from_string_query(build_query)
