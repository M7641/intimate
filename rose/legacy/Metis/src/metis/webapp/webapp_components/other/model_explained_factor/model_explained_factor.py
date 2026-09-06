import os
from pathlib import Path

import dash
import pandas as pd
from jinja2 import Template
from metis.configuration import ExplainModel
from metis.utility_tools.connect import load_dataframe_from_string_query


def return_model_explained_from_db(model_config: ExplainModel) -> pd.DataFrame:
    jinja_params = {
        "schema": os.getenv("OUTPUT_DB_SCHEMA", "TRANSFORM"),
        "model_name": model_config.name,
    }

    path_dir_name = Path(__file__).parent.resolve() / "query.sql.jinja"
    with path_dir_name.open(encoding="utf-8") as file:
        sql_query = file.read()
    sql_query = Template(sql_query).render(**jinja_params)

    output_df = load_dataframe_from_string_query(
        query_str=sql_query,
        try_optimise_memory_useage=True,
    )

    return output_df


def return_model_explained_factor_ui(model_config: ExplainModel):
    data_to_use = return_model_explained_from_db(model_config=model_config)
    metric_value = data_to_use["metric_value"].astype(float).to_numpy()[0]

    return dash.html.Div(
        [
            dash.html.Span(
                [
                    dash.html.H6(
                        f"Model Output Explained Factor: {metric_value: .2%}",
                    ),
                ]
            )
        ]
    )
