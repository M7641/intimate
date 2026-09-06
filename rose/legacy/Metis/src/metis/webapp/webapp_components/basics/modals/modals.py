from typing import Any, Literal

import dash_bootstrap_components as dbc
import dash_mantine_components as dmc
from dash import html


def Button(
    children=None,
    id=None,
    type="primary",
    compact=False,
    disabled=False,
    href=None,
    style=None,
    className=None,
) -> html.Div:
    """
    - id (string):
        An ID to be applied to the element.

    - children (optional):
        The content of the button.

    - type (string; optional):
        The type of the button (ex. primary, secondary, etc). Default = 'primary'

    - compact (boolean; optional):
        Whether to show the button in a compact style. Default = 'False'

    - disabled (boolean; optional):
        Whether the button should be disabled. Default = 'False'

    - href (string; optional):
        The destination of the link if the button is being used as a link.

    - style (dict; optional):
        Any inline styling to be applied to the element.

    - className (string; optional):
        The class to be applied to the element.
    """
    for arg in [compact, disabled]:
        if not isinstance(arg, bool):
            msg = f"`{arg}` should be boolean"
            raise TypeError(msg)

    if not className:
        button_class = f"p-button typography-default-strong p-{type} p-{'compact' if compact else 'default'}"
    else:
        button_class = className

    button_body = dmc.Button(
        id=id,
        children=children,
        disabled=disabled,
        className=button_class,
        style=style,
    )

    return html.Div(
        [
            (
                dmc.Anchor(
                    button_body,
                    id=f"{id}-href",
                    href=href,
                    className="p-button-link",
                    style=(
                        None if not disabled else {"pointer-events": "none"}
                    ),  # initial state
                )
                if href
                else button_body
            )
        ]
    )


def back_button():
    return Button(
        None,
        id={"type": f"{id}-back", "index": 1},
        type="neutral",
        compact=True,
    )


def modal_header(
    id: str | dict,
    title: str | None,
    detail: str | tuple[str, ...] | None,
    show_back: bool | None,
    show_close: bool | None,
) -> dbc.ModalHeader:

    _header_left = html.Div(
        [
            html.Span(title, className="modal-title"),
            html.Span(detail, className="modal-detail"),
        ],
        style={
            "display": "flex",
            "flex": "1 1 100%",
            "flexDirection": "column",
            "gap": "4px",
            "justifyContent": "center",
        },
    )

    if show_back:
        _header_left = html.Div(
            [
                html.Div(back_button()),
                _header_left,
            ],
        )

    _header_right = None

    if show_close:
        _header_right = html.Div(
            Button(
                "x",
                type="neutral",
                id=f"{id}-close-modal",
                style={"width": "48px", "height": "48px"},
            ),
            id=f"{id}-close-holder",
        )

    return dbc.ModalHeader(
        html.Div(
            [_header_left, _header_right],
            style={
                "display": "flex",
                "justifyContent": "space-between",
            },
        ),
        close_button=False,
        style={"display": "block", "border": 0},
        id=f"{id}-header",
    )


def modal_body(id: str | dict, children: list[Any]) -> dbc.ModalBody:
    return dbc.ModalBody(id=f"{id}-body", children=children)


def modal_footer(
    id: str | dict,
    children: list[Any],
    footer_align: Literal["left", "center", "right"] = "right",
) -> dbc.ModalFooter:
    alignment = {"left": "flex-start", "center": "center", "right": "flex-end"}
    return dbc.ModalFooter(
        id=f"{id}-footer",
        children=children,
        style={"border": 0, "justifyContent": alignment[footer_align]},
    )


def return_modal(
    id: str | dict,
    body: list[Any],
    footer: list[Any],
    title: str | None = None,
    detail: str | tuple[str, ...] | None = None,
    footer_align: Literal["left", "center", "right"] = "right",
    size: Literal["sm", "lg", "xl", None] = "lg",
    className: str | None = None,
    show_back: bool | None = None,
    show_close: bool | None = None,
) -> dbc.Modal:
    if show_close is None:
        show_close = True

    return dbc.Modal(
        [
            modal_header(
                id=id,
                title=title,
                detail=detail,
                show_back=show_back,
                show_close=show_close,
            ),
            modal_body(id, children=body),
            modal_footer(id, children=footer, footer_align=footer_align),
        ],
        id=f"{id}",
        centered=True,
        size=size,
        is_open=False,
        className=f"{className}",
        backdrop="static",
        backdropClassName="p-backdrop",
    )
