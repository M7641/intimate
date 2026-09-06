import os
from pathlib import Path

import dash
import pandas as pd
from jinja2 import Template
from metis.configuration import ExplainModel
from metis.the_emporium import the_emporium
from metis.utility_tools.connect import load_dataframe_from_string_query


def create_feature_mapping(dataframe: pd.DataFrame) -> dict:
    feature_mapping = {}
    for i in dataframe[["feature", "feature_value_mapping"]].to_dict("records"):
        feature_mapping[i["feature_value_mapping"]] = i["feature"]
    return feature_mapping


def try_float(value):
    try:
        return float(value)
    except (ValueError, TypeError):
        return value


def create_feature_values(dataframe: pd.DataFrame) -> dict:
    feature_values_dict = {}
    for i in dataframe[["feature_value_mapping", "shap_value", "actual_value"]].to_dict(
        "records"
    ):
        feature_values_dict[f"{i['feature_value_mapping']}"] = {
            "effect": try_float(i["shap_value"]),
            "value": try_float(i["actual_value"]),
        }
    return feature_values_dict


def return_force_plot_from_db(
    model_config: ExplainModel,
    selected_entity_id: str,
) -> pd.DataFrame:
    jinja_params = {
        "schema": os.getenv("OUTPUT_DB_SCHEMA", "TRANSFORM"),
        "selected_entity_id": selected_entity_id,
        "model_name": model_config.name,
    }
    path_dir_name = Path(__file__).parent.resolve() / "query.sql.jinja"
    with Path.open(path_dir_name, encoding="utf-8") as file:
        sql_query = file.read()
    sql_query = Template(sql_query).render(**jinja_params)

    return load_dataframe_from_string_query(
        query_str=sql_query,
        try_optimise_memory_useage=True,
    )


def return_force_plot_ui(
    model_config: ExplainModel,
    selected_entity_id: str,
):
    component_data = return_force_plot_from_db(
        model_config=model_config,
        selected_entity_id=selected_entity_id,
    )

    force_plot = the_emporium.ForcePlot(
        id="force_plot",
        baseValue=component_data["shap_base_rate"].astype(float).iloc[0],
        featureNames=create_feature_mapping(component_data),
        outNames=["Model Output"],
        features=create_feature_values(component_data),
        plot_cmap=["#2A44D4", "#FF3C82"],
        hideBaseValueLabel=True,
    )

    return dash.html.Div(
        [
            dash.html.H3("Force Plot:"),
            dash.html.Div(
                [force_plot],
                className="generic-plots",
            ),
        ],
    )
