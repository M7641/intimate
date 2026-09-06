import dash
from metis.configuration import ExplainModel


def return_model_description_ui(model_config: ExplainModel):

    if model_config.visible_name:
        model_name_visible = model_config.visible_name
    else:
        model_name_visible = " ".join(model_config.name.split("_"))

    model_description_visible = model_config.description

    ui_description = [
        dash.html.Div(
            [
                dash.html.P(
                    "Model:",
                    className="model-description-title",
                    style={
                        "fontWeight": "bold",
                        "marginRight": "4px",
                    },
                ),
                dash.html.P(
                    f"{model_name_visible}", className="model-description-title"
                ),
            ],
            style={"display": "flex", "alignItems": "left"},
        )
    ]

    if model_description_visible != "":
        ui_description.append(
            dash.html.Div(
                [
                    dash.html.P(
                        "Description: ",
                        className="model-description-description",
                        style={"fontWeight": "bold", "marginBottom": "8px"},
                    ),
                    dash.html.P(
                        f"{model_description_visible}",
                        className="model-description-description",
                    ),
                ],
            )
        )

    return dash.html.Div([*ui_description])
