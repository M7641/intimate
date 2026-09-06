from pathlib import Path

import dash
import dash_bootstrap_components as dbc
import pandas as pd
from jinja2 import Template
from metis.configuration import ExplainModel
from metis.utility_tools.connect import load_dataframe_from_string_query
from metis.webapp.plotly_wrapped import (plotly_double_y_axis_bar_chart,
                                         plotly_scatter_plot,
                                         return_graph_config)


def return_model_back_test_from_db(model_config: ExplainModel) -> pd.DataFrame:
    jinja_params = {
        "model_validation_table": model_config.model_validation_table,
        "sample_size": 1000,
    }

    query_name = f"query_{model_config.model_type}.sql.jinja"

    path_dir_name = Path(__file__).parent.resolve() / query_name
    with Path.open(path_dir_name, encoding="utf-8") as file:
        sql_query = file.read()
    sql_query = Template(sql_query).render(**jinja_params)

    output_df = load_dataframe_from_string_query(
        query_str=sql_query,
        try_optimise_memory_useage=True,
    )

    return output_df


def return_model_back_test_ui(model_config: dict):

    plotting_data = return_model_back_test_from_db(model_config=model_config)

    alert_box_ui = dash.html.Div([])
    if model_config.model_type == "binary_classifier":
        figure = plotly_double_y_axis_bar_chart(
            plotting_data=plotting_data.sort_values("score"),
            xaxis_column="score",
            y1_axis_column="hit_rate",
            y2_axis_column="total",
            xaxis_label="",
            y1_axis_label=model_config.output_name_in_plots,
            y2_axis_label=f"Number of {model_config.entity_name}s",
            y1_axis_tickformat=".2%",
            y2_axis_tickformat=".2s",
            hide_ledgend=False,
            use_calculated_range=True,
        )
    elif model_config.model_type == "multi_classifier":
        figure = plotly_double_y_axis_bar_chart(
            plotting_data=plotting_data.sort_values("target"),
            xaxis_column="target",
            y1_axis_column="hit_rate",
            y2_axis_column="total",
            xaxis_label="",
            y1_axis_label=model_config.output_name_in_plots,
            y2_axis_label=f"Number of {model_config.entity_name}s",
            y1_axis_tickformat=".2%",
            y2_axis_tickformat=".2s",
            hide_ledgend=False,
            use_calculated_range=True,
        )
    elif model_config.model_type == "regression":
        plotting_data["target"] = plotting_data["target"].astype(float)
        plotting_data["predicted"] = plotting_data["predicted"].astype(float)
        figure = plotly_scatter_plot(
            plotting_data=plotting_data.sort_values("target"),
            xaxis_column="target",
            yaxis_column="predicted",
            xaxis_label="True value",
            yaxis_label="Predicted value",
            hide_ledgend=False,
        )
        alert_box_ui = dbc.Alert(
            "For performance reasons, we only select a samlpe of random points for this plot!",
            color="info",
            style={"width": "fit-content", "margin": "8px", "padding": "8px"},
        )

    return dash.html.Div(
        [
            dash.html.Div(
                [
                    dash.html.H3(
                        "Model Validation - Hit Rate:",
                        style={"margin": "8px", "padding": "8px"},
                    ),
                    alert_box_ui,
                ],
                style={
                    "display": "flex",
                },
            ),
            dash.dcc.Graph(
                figure=figure, config=return_graph_config(), className="generic-plots"
            ),
        ]
    )
