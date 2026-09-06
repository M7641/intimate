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
from metis.webapp.webapp_components.basics import \
    return_something_went_wrong_ui
from metis.webapp.webapp_components.utils import (
    build_plotting_data_with_quartiles, build_plotting_data_without_quartiles,
    convert_nan_like_values_to_nan)
from natsort import natsort_keygen


def return_hit_rate_across_feature_from_db(
    model_config: ExplainModel,
    feature: str,
    feature_value: str,
    global_feature: str,
    target_col: str,
    start_date_picker: str | None = None,
    end_date_picker: str | None = None,
) -> pd.DataFrame:
    jinja_params = {
        "schema": os.getenv("OUTPUT_DB_SCHEMA", "TRANSFORM"),
        "model_target_table_name": model_config.response_variable_table_name,
        "ucv_ucv_table_name": model_config.table_used_to_explain,
        "entity_id": model_config.entity_id,
        "feature": feature,
        "feature_value": feature_value,
        "global_feature": global_feature,
        "model_name": model_config.name,
        "start_date_picker": start_date_picker,
        "end_date_picker": end_date_picker,
        "response_variable_field_name": model_config.response_variable_field_name,
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

    output_df = output_df.dropna(subset=["actual_value", target_col])

    for function in (
        build_plotting_data_with_quartiles,
        build_plotting_data_without_quartiles,
    ):
        try:
            output_df = function(output_df)
            break
        except ValueError:
            continue
    else:
        output_df["actual_value_bins"] = output_df["actual_value"]
        output_df["actual_value_bins_to_sort"] = output_df["actual_value"]

    output_df = (
        output_df.groupby(
            ["actual_value_bins", "actual_value_bins_to_sort"], observed=True
        )
        .agg(
            {
                target_col: "mean",
                "table_id": np.size,
            },
        )
        .reset_index()
        .rename(columns={"table_id": "Number of Entities"})
        .dropna()
    )

    output_df[global_feature] = output_df["actual_value_bins"].astype(str)

    if output_df.empty:
        raise ValueError("No data returned from query.")

    return output_df


def return_hit_rate_across_feature_ui(
    model_config: dict,
    global_feature: str,
    feature: str | None = None,
    feature_value: str | None = None,
    start_date_picker: str | None = None,
    end_date_picker: str | None = None,
):
    target_col = "target"

    try:
        plotting_data = return_hit_rate_across_feature_from_db(
            model_config=model_config,
            feature=feature,
            feature_value=feature_value,
            global_feature=global_feature,
            target_col=target_col,
            start_date_picker=start_date_picker,
            end_date_picker=end_date_picker,
        )
    except (ValueError, IndexError):
        return return_something_went_wrong_ui()

    figure = plotly_double_y_axis_bar_chart(
        plotting_data=plotting_data.sort_values(
            "actual_value_bins_to_sort", key=natsort_keygen()
        ),
        xaxis_column="actual_value_bins",
        y1_axis_column=target_col,
        y2_axis_column="Number of Entities",
        xaxis_label="",
        y1_axis_label=model_config.output_name_in_plots,
        y2_axis_label=f"Number of {model_config.entity_name}s",
        y1_axis_tickformat=(
            ".2%" if model_config.model_type == "binary_classifier" else None
        ),
        y2_axis_tickformat=".2s",
        hide_ledgend=False,
        use_calculated_range=True,
    )

    return dash.html.Div(
        [
            dash.html.H3("Hit Rate Across Feature Values:"),
            dash.dcc.Graph(
                figure=figure, config=return_graph_config(), className="generic-plots"
            ),
        ]
    )
