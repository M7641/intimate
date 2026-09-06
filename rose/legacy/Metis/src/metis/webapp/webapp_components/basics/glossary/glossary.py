from pathlib import Path

import dash
import dash_bootstrap_components as dbc
from metis.configuration.config_files import return_glossary


def build_glossary(glossary_data):
    build_records = list()

    for i in glossary_data.glossary:
        build_records.append({"name": i.name, "definition": i.definition})

    build_records = sorted(build_records, key=lambda x: x["name"])
    return build_records


def return_glossary_ui():
    show_glossary = True
    try:
        glossary_data = return_glossary(
            path=Path.cwd().resolve(),
            file_name="glossary.yaml",
        )
        glossary_data = build_glossary(glossary_data)
    except Exception:
        show_glossary = False
        glossary_data = {"name": "No Glossary Found", "definition": "No Glossary Found"}

    if show_glossary:
        conditional_style = {"display": "block"}
    else:
        conditional_style = {"display": "none"}

    cell_style = {
        "textAlign": "left",
        "fontFamily": "sans-serif",
        "whiteSpace": "normal",
        "height": "auto",
    }
    cell_style_conditional = [
        {"if": {"column_id": "field_clean"}, "fontWeight": "bold", "width": "50%"},
        {"if": {"column_id": "value"}, "width": "50%"},
    ]

    data_table_glossary = dash.dash_table.DataTable(
        data=glossary_data,
        columns=[
            {"name": "Feature", "id": "name"},
            {"name": "Definition", "id": "definition"},
        ],
        style_cell=cell_style,
        style_cell_conditional=cell_style_conditional,
        style_table={"overflowX": "auto"},
        style_header={
            "backgroundColor": "white",
            "fontWeight": "bold",
            "textAlign": "left",
            "fontFamily": "sans-serif",
        },
        cell_selectable=False,
        row_selectable=False,
        style_as_list_view=True,
    )

    ui_component = dash.html.Div(
        [
            dbc.Button(
                "Glossary",
                id="open-offcanvas-scrollable",
                n_clicks=0,
                class_name="button_white",
            ),
            dbc.Offcanvas(
                dash.html.Div([data_table_glossary]),
                id="offcanvas-scrollable",
                scrollable=True,
                title="Glossary",
                is_open=False,
                placement="end",
                backdrop=False,
                style={"width": "40%"},
            ),
        ],
        style={"marginTop": "8px", "float": "right", **conditional_style},
    )

    return ui_component
