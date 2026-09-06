import logging
import math
from pathlib import Path

import dash
import pandas as pd
from jinja2 import Template
from metis.configuration import ExplainModel
from metis.utility_tools.connect import load_dataframe_from_string_query


def return_ucv_table_view_per_entity_from_db(
    model_config: ExplainModel,
    selected_entity_id: str,
) -> pd.DataFrame:
    jinja_params = {
        "ucv_ucv_table_name": model_config.table_used_to_explain,
        "fields_to_return": model_config.fields_to_inlcude_in_individual_table,
        "selected_entity_id": selected_entity_id,
        "entity_id": model_config.entity_id,
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

    return output_df


def return_ucv_table_view_per_entity_ui(
    model_config: ExplainModel,
    selected_entity_id: str,
):
    component_data = return_ucv_table_view_per_entity_from_db(
        model_config=model_config,
        selected_entity_id=selected_entity_id,
    )

    component_data = component_data[["field_clean", "value"]]
    top_half_ucv = component_data.head(math.ceil(len(component_data) / 2))
    bottom_half_ucv = component_data.tail(math.floor(len(component_data) / 2))

    cell_style = {
        "textAlign": "left",
        "border": "none",
        "fontFamily": "sans-serif",
        "whiteSpace": "normal",
        "height": "auto",
        "backgroundColor": "#FFFFFF",
    }
    cell_style_conditional = [
        {"if": {"column_id": "field_clean"}, "fontWeight": "bold", "width": "50%"},
        {"if": {"column_id": "value"}, "width": "50%"},
    ]

    data_table_one = dash.dash_table.DataTable(
        id="ucv_table_view_per_entity_one",
        data=top_half_ucv.to_dict("records"),
        columns=[{"name": i, "id": i} for i in top_half_ucv.columns],
        style_cell=cell_style,
        style_cell_conditional=cell_style_conditional,
        style_header={
            "display": "none",
            "height": "0px",
        },
        style_table={"overflowX": "auto"},
        cell_selectable=False,
        row_selectable=False,
    )
    data_table_two = dash.dash_table.DataTable(
        id="ucv_table_view_per_entity_two",
        data=bottom_half_ucv.to_dict("records"),
        columns=[{"name": i, "id": i} for i in bottom_half_ucv.columns],
        style_cell=cell_style,
        style_cell_conditional=cell_style_conditional,
        style_header={
            "display": "none",
            "height": "0px",
        },
        style_table={"overflowX": "auto"},
        cell_selectable=False,
        row_selectable=False,
    )

    return dash.html.Div(
        [
            dash.html.H3(f"{model_config.entity_name} details:"),
            dash.html.Div(
                [data_table_one, data_table_two],
                className="data_table_box",
                id="entity_breakdown_box",
            ),
        ],
    )
