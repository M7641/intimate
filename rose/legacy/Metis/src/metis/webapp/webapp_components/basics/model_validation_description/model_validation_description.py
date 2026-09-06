import dash


def return_model_validation_description_ui():
    ui_description = [
        dash.html.Div(
            [
                dash.html.P(
                    "How we validate models:",
                    className="model-description-title",
                    style={
                        "fontWeight": "bold",
                        "marginRight": "4px",
                        "marginBottom": "0px",
                        "height": "30px",
                    },
                ),
            ],
            style={"display": "flex", "alignItems": "left", "marginBottom": "4px"},
        )
    ]

    text_to_disaplay = """
    We validate models by hiding recent data from the model
    and then asking what will happen. This compares the model predictions with reality
    and shows how well the model has learnt.

    If a model has been well-trained, we expect to see a monotonic upward trend
    from low to high scores. If the model is poorly trained, we
    see a flat trend or appear somewhat random.
    """

    ui_description.append(
        dash.html.Div(
            [
                dash.html.Span(
                    f"{text_to_disaplay}",
                    className="model-description-description",
                ),
            ],
        )
    )

    return dash.html.Div([*ui_description])
