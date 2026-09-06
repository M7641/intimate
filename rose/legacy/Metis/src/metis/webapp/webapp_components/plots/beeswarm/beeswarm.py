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
from metis.webapp.webapp_components.utils import (
    convert_nan_like_values_to_nan, try_float)


def return_beeswarm_data_from_db(
    model_config: ExplainModel,
    feature: str | None = None,
    feature_value: str | None = None,
    start_date_picker: str | None = None,
    end_date_picker: str | None = None,
    sample_size: int = 400,
    number_of_features: int = 10,
) -> pd.DataFrame:
    jinja_params = {
        "schema": os.getenv("OUTPUT_DB_SCHEMA", "TRANSFORM"),
        "ucv_ucv_table_name": model_config.table_used_to_explain,
        "sample_size": sample_size,
        "number_of_features": number_of_features,
        "feature": feature,
        "feature_value": feature_value,
        "model_name": model_config.name,
        "start_date_picker": start_date_picker,
        "end_date_picker": end_date_picker,
        "entity_id": model_config.entity_id,
    }
    path_dir_name = Path(__file__).parent.resolve() / "query.sql.jinja"
    with Path.open(path_dir_name, encoding="utf-8") as file:
        sql_query = file.read()
    sql_query = Template(sql_query).render(**jinja_params)

    output_df = load_dataframe_from_string_query(
        query_str=sql_query,
        try_optimise_memory_useage=True,
    )
    output_df = output_df.map(lambda x: convert_nan_like_values_to_nan(x))

    output_df["actual_value_for_color"] = (
        output_df["actual_value"].apply(try_float).rank(pct=True)
    )
    output_df["shap_value"] = output_df["shap_value"].apply(try_float)
    output_df["shap_ordering"] = output_df["shap_value"].abs()

    return output_df


def return_beeswarm_ui(
    model_config: ExplainModel,
    feature: str | None = None,
    feature_value: str | None = None,
    start_date_picker: str | None = None,
    end_date_picker: str | None = None,
):
    plotting_data = return_beeswarm_data_from_db(
        model_config=model_config,
        feature=feature,
        feature_value=feature_value,
        start_date_picker=start_date_picker,
        end_date_picker=end_date_picker,
    )

    ordering = (
        plotting_data.groupby(["feature_clean"], observed=True)
        .mean(numeric_only=True)["shap_ordering"]
        .nsmallest(10)
        .index
    )

    fig = go.Figure()
    N = len(plotting_data)
    y_axis_tick_values = []
    trace_gap = 0
    colour_bar_done = None

    if model_config.model_type == "binary_classifier":
        plotting_data["shap_value"] = plotting_data["shap_value"] * 100

    for i in ordering:
        plot = plotting_data.loc[plotting_data["feature_clean"] == i]
        take_min = plot["actual_value_for_color"].min()
        take_max = plot["actual_value_for_color"].max()

        if (
            plot["actual_value_for_color"].isna().sum() == 0
            and colour_bar_done is None
            and take_min != take_max
        ):
            colour_bar_config = {
                "title": {"text": "Feature Value", "side": "right"},
                "tickmode": "array",
                "tickvals": [take_min, take_max],
                "ticktext": ["Low", "High"],
                "thicknessmode": "pixels",
                "thickness": 5,
                "ticks": "outside",
                "orientation": "v",
            }
            colour_bar_done = "Done"
        else:
            colour_bar_config = None

        colour_array = (
            "#d3d3d3"
            if plot["actual_value_for_color"].isna().sum() > 0
            else plot["actual_value_for_color"]
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

        fig.add_trace(
            go.Scattergl(
                x=plot["shap_value"],
                y=trace_gap + np.random.normal(0, 0.08, N),
                mode="markers",
                marker={
                    "size": 5,
                    "color": colour_array,
                    "colorscale": ["#2A44D4", "#FF3C82"],
                    "colorbar": colour_bar_config,
                },
                name=i,
                customdata=np.stack(
                    (
                        plot["table_id"],
                        plot["shap_value"],
                        plot["actual_value"],
                    ),
                    axis=-1,
                ),
                hovertemplate=hovertemplate,
            ),
        )
        y_axis_tick_values.append(trace_gap)
        trace_gap += 1.2

    fig.update_layout(
        showlegend=False,
        coloraxis_showscale=True,
        plot_bgcolor="white",
        margin={"l": 20, "r": 20, "t": 20, "b": 20, "pad": 10},
    )
    fig.update_yaxes(
        tickvals=y_axis_tick_values,
        ticktext=[i.replace("_", " ").capitalize() for i in ordering],
        showgrid=True,
        gridwidth=1,
        gridcolor="#D3D3D3",
        griddash="2px",
        ticks="outside",
        tickfont={"size": 13},
    )
    fig.update_xaxes(
        showgrid=False,
        title_text="Change in model output",
        ticks="outside",
        ticksuffix="%" if model_config.model_type == "binary_classifier" else "",
    )
    fig.add_hline(
        y=0,
        line_width=1,
        line_color="grey",
        opacity=0.25,
        line_dash="2px",
    )
    fig.add_vline(
        x=0,
        line_width=2,
        line_color="grey",
        opacity=0.5,
    )
    fig.update_coloraxes(colorbar_title_side="right")

    return dash.html.Div(
        [
            dash.html.H3("Beeswarm Plot:"),
            # dash.html.P("Plot Description ..."),
            dash.dcc.Graph(
                figure=fig,
                config=return_graph_config(),
                className="generic-plots",
            ),
        ],
    )
