import os
from pathlib import Path

import dash
import numpy as np
import pandas as pd
from jinja2 import Template
from metis.configuration import ExplainModel
from metis.utility_tools.connect import load_dataframe_from_string_query
from metis.webapp.plotly_wrapped import (plotly_double_y_axis_bar_chart,
                                         return_graph_config)


def return_most_important_feature_per_entity_from_db(
    model_config: ExplainModel,
    selected_entity_id: str,
) -> pd.DataFrame:
    jinja_params = {
        "schema": os.getenv("OUTPUT_DB_SCHEMA", "TRANSFORM"),
        "selected_entity_id": selected_entity_id,
        "model_name": model_config.name,
        "number_of_features": 6,
    }
    path_dir_name = Path(__file__).parent.resolve() / "query.sql.jinja"
    with Path.open(path_dir_name, encoding="utf-8") as file:
        sql_query = file.read()
    sql_query = Template(sql_query).render(**jinja_params)

    output_df = load_dataframe_from_string_query(
        query_str=sql_query,
        try_optimise_memory_useage=True,
    )

    return output_df


def return_most_important_feature_per_entity_ui(
    model_config: ExplainModel,
    selected_entity_id: str,
):
    plotting_data = return_most_important_feature_per_entity_from_db(
        model_config=model_config,
        selected_entity_id=selected_entity_id,
    )

    if model_config.model_type == "binary_classifier":
        hovertemplate = (
            "<br><b>Feature</b>: %{customdata[0]}"
            "<br><b>Change in Model Output (%)</b>: %{customdata[1]:,.2f}"
            "<br><b>Feature Value</b>: %{customdata[2]}"
            "<extra></extra>"
        )
    else:
        hovertemplate = (
            "<br><b>Feature</b>: %{customdata[0]}"
            "<br><b>Change in Model Output</b>: %{customdata[1]:,.2f}"
            "<br><b>Feature Value</b>: %{customdata[2]}"
            "<extra></extra>"
        )

    figure = plotly_double_y_axis_bar_chart(
        plotting_data=plotting_data,
        xaxis_column="shap_value",
        y1_axis_column="feature_clean",
        y2_axis_column=None,
        xaxis_label="Change in Model output",
        y1_axis_label="Features",
        y2_axis_label="",
        y1_axis_tickformat=".2f",
        y2_axis_tickformat=".2s",
        hide_ledgend=False,
        use_calculated_range=True,
        orientation="h",
        custom_data=np.stack(
            (
                plotting_data["feature_clean"],
                plotting_data["shap_value"],
                plotting_data["actual_value"],
            ),
            axis=-1,
        ),
        hovertemplate=hovertemplate,
    )

    figure.update_xaxes(
        showgrid=False,
        ticks="outside",
        ticksuffix="%" if model_config.model_type == "binary_classifier" else "",
    )

    return dash.html.Div(
        [
            dash.html.H3("Most Important features:"),
            dash.dcc.Graph(
                figure=figure,
                config=return_graph_config(),
                className="generic-plots",
            ),
        ],
    )
