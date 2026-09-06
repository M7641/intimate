from pathlib import Path

import dash
import dash_bootstrap_components as dbc
from dash_extensions.enrich import DashProxy, ServersideOutputTransform, html
from metis.webapp.webapp_utils import get_prefix

assets_path = str(Path(__file__).parent.resolve() / "assets/")

external_stylesheets = [
    dbc.icons.BOOTSTRAP,
    dbc.themes.BOOTSTRAP,
    dbc.icons.FONT_AWESOME,
]

web_app = DashProxy(
    name=__name__,
    external_stylesheets=external_stylesheets,
    requests_pathname_prefix=get_prefix(),
    use_pages=True,
    suppress_callback_exceptions=True,
    transforms=[ServersideOutputTransform()],
    assets_folder=assets_path,
    serve_locally=True,
)

web_app.layout = html.Div(
    [
        dbc.Container(dash.page_container, fluid=True),
    ],
)

server = web_app.server
