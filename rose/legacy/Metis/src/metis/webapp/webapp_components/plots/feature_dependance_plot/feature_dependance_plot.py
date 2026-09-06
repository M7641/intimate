import os
from pathlib import Path

import dash
import numpy as np
import pandas as pd
import plotly.graph_objects as go
from jinja2 import Template
from metis.configuration import ExplainModel
from metis.utility_tools.connect import load_dataframe_from_string_query
from metis.webapp.plotly_wrapped import return_graph_config


def return_feature_dependency_plot_from_db(
    model_config: ExplainModel,
    feature_one: str,
    feature_two: str,
    feature: str | None = None,
    feature_value: str | None = None,
    sample_size: int = 500,
) -> pd.DataFrame:
    jinja_params = {
        "schema": os.getenv("OUTPUT_DB_SCHEMA", "TRANSFORM"),
        "ucv_ucv_table_name": model_config.table_used_to_explain,
        "sample_size": sample_size,
        "feature": feature,
        "feature_value": feature_value,
        "model_name": model_config.name,
        "entity_id": model_config.entity_id,
        "feature_one": feature_one,
        "feature_two": feature_two,
    }
    path_dir_name = Path(__file__).parent.resolve() / "query.sql.jinja"
    with Path.open(path_dir_name, encoding="utf-8") as file:
        sql_query = file.read()
    sql_query = Template(sql_query).render(**jinja_params)

    output_df = load_dataframe_from_string_query(
        query_str=sql_query,
        try_optimise_memory_useage=True,
    ).dropna()

    try:
        output_df[feature_one] = output_df[feature_one].astype(float)
    except ValueError:
        pass

    return output_df


def return_feature_dependency_plot_ui(
    model_config: ExplainModel,
    feature_one: str,
    feature_two: str,
    feature: str | None = None,
    feature_value: str | None = None,
):
    plotting_data = return_feature_dependency_plot_from_db(
        model_config=model_config,
        feature=feature,
        feature_value=feature_value,
        feature_one=feature_one,
        feature_two=feature_two,
    )

    try:
        plotting_data[feature_two] = plotting_data[feature_two].astype(float)
        plotting_data["colour_mapping"] = plotting_data[feature_two]
    except ValueError:
        plotting_data[feature_two] = plotting_data[feature_two].astype(str)
        plotting_data[feature_two] = pd.Categorical(plotting_data[feature_two])
        plotting_data["colour_mapping"] = plotting_data[feature_two].cat.codes

    figure = go.Figure()

    figure.add_trace(
        go.Scatter(
            x=plotting_data[feature_one],
            y=plotting_data["shap_value"],
            mode="markers",
            marker={
                "size": 5,
                "color": plotting_data["colour_mapping"],
                "colorscale": ["#2A44D4", "#FF3C82"],
            },
            customdata=np.stack(
                (
                    plotting_data[feature_one],
                    plotting_data[feature_two],
                    plotting_data["shap_value"],
                ),
                axis=-1,
            ),
            hovertemplate=(
                "<br><b>Primary Feature</b>: %{customdata[0]}"
                "<br><b>Secondary Feature</b>: %{customdata[1]}"
                "<br><b>SHAP value</b>: %{customdata[2]:,.1f}"
                "<extra></extra>"
            ),
            showlegend=False,
        )
    )

    figure.update_yaxes(
        title_text="SHAP value",
    )
    figure.update_xaxes(
        title_text=feature_one.replace("_", " ").capitalize(),
    )
    figure.update_layout(
        showlegend=False,
        plot_bgcolor="white",
        margin={"l": 20, "r": 20, "t": 20, "b": 20, "pad": 10},
    )
    figure.update_layout(coloraxis_showscale=False)

    return dash.html.Div(
        [
            # dash.html.P("Plot Description ..."),
            dash.dcc.Graph(
                figure=figure,
                config=return_graph_config(),
                className="generic-plots",
            ),
        ],
    )
