import math

import numpy as np
import pandas as pd
import plotly.graph_objects as go
import plotly.io as pio


def get_brand_colors():
    """
    Return a list of brand colors.
    """
    return [
        "#2A44D4",
        "#041537",
        "#FFDB21",
        "#FF3C82",
        "#800080",
        "#73F692",
        "#A600FF",
        "#FF78A8",
        "#6A7CE1",
        "#9DF8B2",
        "#9BA1AF",
        "#FF9EC1",
        "#D280FF",
        "#041537",
        "#95A1E9",
        "#B8FAC8",
        "#FFEC90",
        "#000000",
        "#808080",
        "#999999",
        "#F4F6FD",
        "#CAD0F4",
    ]


def return_graph_config():
    """
    Returns a dictionary representing the configuration options for a graph.

    :return: A dictionary with the following keys:
             - "displaylogo" (bool): Whether or not to display the plotly logo.
             - "toImageButtonOptions" (dict): Options for saving the graph as an image.
                 - "format" (str): The format of the image (e.g., "jpeg", "png").
                 - "filename" (str): The filename for the saved image.
                 - "height" (int): The height of the saved image in pixels.
                 - "width" (int): The width of the saved image in pixels.
                 - "scale" (int): The scale factor for the saved image.
             - "modeBarButtonsToRemove" (list): A list of mode bar buttons to remove.
               Possible values include "zoom", "pan", "zoomIn", "zoomOut", "autoScale",
               "zoomIn2d", "zoomOut2d", "lasso2d", "select2d".

    """
    return {
        "displaylogo": False,
        "toImageButtonOptions": {
            "format": "jpeg",
            "filename": "custom_image",
            "height": 720,
            "width": 720,
            "scale": 1,
        },
        "modeBarButtonsToRemove": [
            "zoom",
            "pan",
            "zoomIn",
            "zoomOut",
            "autoScale",
            "zoomIn2d",
            "zoomOut2d",
            "lasso2d",
            "select2d",
        ],
    }


def return_plotly_style():
    my_template = pio.templates["plotly"]
    my_template.layout.update(
        {
            "paper_bgcolor": "#FFFFFF",
            "plot_bgcolor": "#FFFFFF",
            "showlegend": True,
            "autosize": True,
            "xaxis": {
                "showline": False,
                "showgrid": True,
                "tickson": "boundaries",
                "showticklabels": True,
                "ticks": "outside",
                "tickfont": {
                    "size": 12,
                },
                "linecolor": "#000000",
                "linewidth": 2,
            },
            "yaxis": {
                "showline": False,
                "showgrid": True,
                "showticklabels": True,
                "ticks": "outside",
                "tickfont": {
                    "size": 12,
                },
                "linecolor": "#000000",
                "linewidth": 2,
            },
            "title": {
                "font": {"size": 16},
            },
            "margin": {
                "t": 10,
                "b": 10,
                "l": 16,
                "r": 16,
            },
        },
    )
    return my_template


def plotly_double_y_axis_bar_chart(
    plotting_data: pd.DataFrame,
    xaxis_column: str,
    y1_axis_column: str,
    y2_axis_column: str | None,
    xaxis_label: str = "XAxis",
    y1_axis_label: str = "Y1Axis",
    y2_axis_label: str | None = "Y2Axis",
    y1_axis_tickformat: str = ".0%",
    y2_axis_tickformat: str = ".0%",
    hide_ledgend: bool = False,
    use_calculated_range: bool = True,
    custom_y2_tick_setting: dict | None = None,
    orientation="v",
    hovertemplate: str | None = None,
    custom_data: np.ndarray | None = None,
):
    plots = [
        go.Bar(
            x=plotting_data[xaxis_column],
            y=plotting_data[y1_axis_column],
            name=y1_axis_label,
            yaxis="y",
            offsetgroup=1,
            marker={"color": get_brand_colors()[0]},
            orientation=orientation,
            customdata=custom_data,
            hovertemplate=hovertemplate,
        ),
    ]
    if y2_axis_column is not None:
        plots = [
            *plots,
            go.Bar(
                x=plotting_data[xaxis_column],
                y=plotting_data[y2_axis_column],
                name=y2_axis_label,
                yaxis="y2",
                offsetgroup=2,
                marker={"color": get_brand_colors()[1]},
            ),
        ]

    fig = go.Figure(data=plots)

    fig.update_layout(
        template=return_plotly_style(),
        barmode="group",
        xaxis_title=xaxis_label,
        legend_title="",
        showlegend=bool(y2_axis_column is not None),
        yaxis={"title": {"text": y1_axis_label}, "tickformat": y1_axis_tickformat},
    )

    if y2_axis_column is not None:
        max_value = max(plotting_data[y2_axis_column])
        magnitude = math.floor(math.log(abs(max_value), 10))
        y_max = round(max_value + 10**magnitude, -magnitude)

        if min(plotting_data[y2_axis_column]) >= 0:
            y_min = 0
        else:
            min_value = min(plotting_data[y2_axis_column])
            magnitude_min = math.floor(math.log(abs(min_value), 10))
            y_min = round(min_value - 10**magnitude_min, -magnitude_min)

        if custom_y2_tick_setting is None:
            yaxis2_layout = {
                "title": {"text": y2_axis_label},
                "side": "right",
                "range": [y_min, y_max] if use_calculated_range else None,
                "overlaying": "y",
                "tickmode": "sync",
                "tickformat": y2_axis_tickformat,
            }
        else:
            yaxis2_layout = custom_y2_tick_setting

        fig.update_layout(
            yaxis={
                "side": "left",
            },
            yaxis2=yaxis2_layout,
            legend={
                "orientation": "h",
                "yanchor": "top",
                "y": 1.05,
                "xanchor": "left",
                "x": 0.4,
            },
        )

    if hide_ledgend:
        fig.update_layout(showlegend=False)

    return fig


def plotly_scatter_plot(
    plotting_data: pd.DataFrame,
    xaxis_column: str,
    yaxis_column: str,
    xaxis_label: str = "XAxis",
    yaxis_label: str = "Y1Axis",
    hide_ledgend: bool = False,
    hovertemplate: str | None = None,
    custom_data: np.ndarray | None = None,
):
    figure = go.Figure()
    figure.add_trace(
        go.Scatter(
            x=plotting_data[xaxis_column],
            y=plotting_data[yaxis_column],
            mode="markers",
            marker={"size": 5},
            customdata=custom_data,
            hovertemplate=hovertemplate,
            showlegend=hide_ledgend,
        )
    )

    figure.update_yaxes(title_text=yaxis_label)
    figure.update_xaxes(
        title_text=xaxis_label,
    )
    figure.update_layout(
        plot_bgcolor="white",
        margin={"l": 20, "r": 20, "t": 20, "b": 20, "pad": 10},
    )
    return figure
