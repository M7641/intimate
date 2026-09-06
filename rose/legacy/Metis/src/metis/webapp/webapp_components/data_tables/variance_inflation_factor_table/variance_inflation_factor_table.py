import logging
import os
import warnings
from pathlib import Path

import dash
import pandas as pd
from jinja2 import Template
from metis.configuration import ExplainModel
from metis.utility_tools.connect import load_dataframe_from_string_query
from metis.webapp.fetch_data import return_valid_fields_to_explain_with
from metis.webapp.webapp_components.utils import clean_feature_names
from statsmodels.stats.outliers_influence import variance_inflation_factor


def compute_vif(dataframe: pd.DataFrame, considered_features: list) -> pd.DataFrame:
    dataframe_copy = dataframe.copy()[considered_features].dropna(
        thresh=0.7 * len(dataframe), axis=1
    )
    dataframe_copy["intercept"] = 1

    vif = pd.DataFrame()
    vif["variable"] = dataframe_copy.columns

    with warnings.catch_warnings():
        warnings.filterwarnings("ignore")
        vif["vif"] = [
            variance_inflation_factor(dataframe_copy.values, i)
            for i in range(dataframe_copy.shape[1])
        ]

    vif = vif[vif["variable"] != "intercept"]

    return vif


def return_variance_inflation_factor_table_from_db(
    model_config: ExplainModel,
) -> pd.DataFrame:
    """Grey out the 0s and make it clear they don't exists."""

    fields_to_explain_with = return_valid_fields_to_explain_with()
    jinja_params = {
        "schema": os.getenv("OUTPUT_DB_SCHEMA", "TRANSFORM"),
        "model_name": model_config.name,
        "fields_to_explain_with": fields_to_explain_with,
    }
    path_dir_name = Path(__file__).parent.resolve() / "query.sql.jinja"

    try:
        with Path.open(path_dir_name, encoding="utf-8") as file:
            sql_query = file.read()
    except FileNotFoundError:
        logging.exception(f"File not found: {path_dir_name}")
        return pd.DataFrame()

    sql_query = Template(sql_query).render(**jinja_params)

    try:
        output_df = load_dataframe_from_string_query(
            query_str=sql_query,
            try_optimise_memory_useage=True,
        )
    except Exception as e:
        logging.exception(f"Error executing SQL query: {e}")
        return pd.DataFrame()

    output_df.columns = map(str, output_df.columns)

    vif_df = compute_vif(output_df, fields_to_explain_with)

    vif_df["variable"] = clean_feature_names(vif_df["variable"])

    vif_df["vif"] = vif_df["vif"].astype(float)
    vif_df["vif"] = vif_df["vif"].round(3)
    vif_df["vif"] = vif_df["vif"].fillna(0.0)

    return vif_df.sort_values("vif", ascending=False)


def return_variance_inflation_factor_table_ui(
    model_config: ExplainModel,
):
    component_data = return_variance_inflation_factor_table_from_db(
        model_config=model_config,
    )

    cell_style = {
        "fontFamily": "sans-serif",
        "whiteSpace": "normal",
        "height": "auto",
        "backgroundColor": "#FFFFFF",
    }

    cell_style_conditional = [
        {
            "if": {"column_id": "variable"},
            "fontWeight": "bold",
            "width": "100%",
            "textAlign": "left",
        },
        {
            "if": {"column_id": "vif"},
            "min-width": "150px",
            "textAlign": "center",
        },
    ]
    cell_data_conditional = [
        {
            "if": {
                "filter_query": "{vif} >= 5",
                "column_id": "vif",
            },
            "backgroundColor": "#ff6961",
        },
        {
            "if": {
                "filter_query": "{vif} <= 2",
                "column_id": "vif",
            },
            "backgroundColor": "#77dd77",
        },
        {
            "if": {
                "filter_query": "{vif} > 2 && {vif} < 5",
                "column_id": "vif",
            },
            "backgroundColor": "#fdfd96",
        },
    ]

    vif_table_component = dash.dash_table.DataTable(
        id="vif_table",
        data=component_data.to_dict("records"),
        columns=[
            {"name": "Variable", "id": "variable"},
            {"name": "VIF", "id": "vif"},
        ],
        style_cell=cell_style,
        style_cell_conditional=cell_style_conditional,
        style_data_conditional=cell_data_conditional,
        style_header={
            "backgroundColor": "white",
            "fontWeight": "bold",
        },
        style_table={"overflowX": "auto"},
        cell_selectable=False,
        row_selectable=False,
    )

    return dash.html.Div(
        [
            dash.html.Div(
                [vif_table_component],
                className="data_table_box",
            ),
        ],
    )
