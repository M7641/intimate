import dash


def return_something_went_wrong_ui():
    css_style = {
        "margin": "auto",
        "width": "fit-content",
        "textAlign": "center",
        "paddingTop": "10%",
    }

    return dash.html.Div(
        [
            dash.html.Img(
                src="assets/warn.svg",
                height="148px",
            ),
            dash.html.H1("Something went wrong!"),
        ],
        style=css_style,
    )
