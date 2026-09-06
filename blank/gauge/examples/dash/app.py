"""A small Plotly Dash app — a realistic thing to point gauge at.

Two reasons Dash is a good example target:
  - Its *baseline* RSS is large: importing dash + plotly + flask pulls in a lot
    before serving a single request. For a real app, that baseline often
    dominates capacity planning more than per-request cost does.
  - It is a normal Flask app underneath (`app.server`), so we can bolt on a
    `/work` route that does real CPU + memory work — something for the load
    driver to bite on, since hitting the cached layout at `/` is nearly free.

Run it directly with `uv run --group examples python examples/dash/app.py`,
or let gauge launch it (see examples/dash/README.md).
"""

import hashlib
import logging
import os
import sys

import plotly.graph_objects as go
from dash import Dash, dcc, html

# Silence Flask's per-request access log so gauge's summary stays readable.
logging.getLogger("werkzeug").setLevel(logging.ERROR)

app = Dash(__name__)
server = app.server  # the underlying Flask app

# A trivial static figure so the page has something to render.
_figure = go.Figure(data=[go.Bar(x=["a", "b", "c"], y=[3, 1, 2])])

app.layout = html.Div(
    [
        html.H2("gauge demo — Dash"),
        html.P("Baseline footprint is mostly imports. Load hits /work."),
        dcc.Graph(figure=_figure),
    ]
)

_cache: list[bytearray] = []  # grows on every /work request → visible RSS climb


@server.route("/work")
def work():
    """CPU + memory work, so the load driver produces a real footprint."""
    block = b"x" * 64_000
    digest = block
    for _ in range(300):
        digest = hashlib.sha256(digest + block).digest()
    _cache.append(bytearray(256_000))
    return f"ok cache={len(_cache)} last={digest[:8].hex()}\n"


def main() -> None:
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8050
    # Bind localhost by default; inside a container set GAUGE_HOST=0.0.0.0 so a
    # published port can reach it.
    host = os.environ.get("GAUGE_HOST", "127.0.0.1")
    app.run(host=host, port=port, debug=False)


if __name__ == "__main__":
    main()
