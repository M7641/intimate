import os
from pathlib import Path

import dash
import numpy as np
import pandas as pd
import plotly.graph_objects as go
from jinja2 import Template
from metis.configuration import ExplainModel
from metis.utility_tools.connect import load_dataframe_from_string_query
from metis.webapp.fetch_data import return_valid_fields_to_explain_with
from metis.webapp.plotly_wrapped import return_graph_config
from metis.webapp.webapp_components.utils import clean_feature_names


def return_r_squared_matrix_from_db(
    model_config: ExplainModel,
    feature: str | None = None,
    feature_value: str | None = None,
    start_date_picker: str | None = None,
    end_date_picker: str | None = None,
) -> pd.DataFrame:

    fields_to_explain_with = return_valid_fields_to_explain_with()

    # future option: https://docs.snowflake.com/en/sql-reference/functions/regr_r2
    jinja_params = {
        "schema": os.getenv("OUTPUT_DB_SCHEMA", "TRANSFORM"),
        "feature": feature,
        "feature_value": feature_value,
        "model_name": model_config.name,
        "fields_to_explain_with": fields_to_explain_with,
        "start_date_picker": start_date_picker,
        "end_date_picker": end_date_picker,
        "ucv_ucv_table_name": model_config.table_used_to_explain,
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

    output_df = output_df.dropna(thresh=0.5 * len(output_df), axis=1)

    output_df = output_df.dropna().corr(numeric_only=True)

    return output_df


def return_r_squared_matrix_ui(
    model_config: dict,
    feature: str | None = None,
    feature_value: str | None = None,
    start_date_picker: str | None = None,
    end_date_picker: str | None = None,
):
    plotting_data = return_r_squared_matrix_from_db(
        model_config=model_config,
        feature=feature,
        feature_value=feature_value,
        start_date_picker=start_date_picker,
        end_date_picker=end_date_picker,
    )

    r_squared_matrix = np.tril(plotting_data.values, k=0)
    r_squared_matrix[r_squared_matrix == 0] = np.nan

    heat_map = go.Heatmap(
        z=r_squared_matrix,
        x=clean_feature_names(plotting_data.columns),
        y=clean_feature_names(plotting_data.columns),
        xgap=1,
        ygap=1,
        colorscale=[
            "#2A44D4",
            "#FF3C82",
        ],
        text=plotting_data.values,
        texttemplate="%{text:.2f}",
        hovertemplate=(
            "<br><b>Feature One</b>: %{x}"
            "<br><b>Feature Two</b>: %{y}"
            "<br><b>Correlation</b>: %{z:,.2f}"
            "<extra></extra>"
        ),
    )

    figure = go.Figure(data=[heat_map])

    figure.update_layout(
        title_automargin=False,
        title_x=0,
        xaxis_showgrid=False,
        yaxis_showgrid=False,
        yaxis_autorange="reversed",
        margin=dict(t=50, b=50),  # r, l, b, t
        height=800,
    )

    figure.update_traces(showscale=False)

    figure.update_xaxes(
        tickangle=45,
    )

    return dash.html.Div(
        [
            dash.dcc.Graph(
                figure=figure, config=return_graph_config(), className="generic-plots"
            )
        ]
    )
