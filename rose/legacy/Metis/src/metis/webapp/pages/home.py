import datetime
import logging
from pathlib import Path

import dash_bootstrap_components as dbc
from dash import clientside_callback, ctx, no_update, register_page
from dash_extensions.enrich import Input, Output, State, callback, dcc, html
from metis.configuration.config_files import return_configuration_file
from metis.utility_tools.hermes import (check_for_api_key,
                                        return_hermes_sage_advice)
from metis.webapp.fetch_data import (return_entity_ids,
                                     return_valid_attribute_values,
                                     return_valid_attributes_to_filter,
                                     return_valid_fields_to_explain_with)
from metis.webapp.webapp_components.basics import (
    return_glossary_ui, return_model_description_ui,
    return_model_validation_description_ui, return_modal,
    return_something_went_wrong_ui)
from metis.webapp.webapp_components.data_tables import (
    return_ucv_table_view_per_entity_ui,
    return_variance_inflation_factor_table_ui)
from metis.webapp.webapp_components.other import \
    return_model_explained_factor_ui
from metis.webapp.webapp_components.plots import (
    return_average_shap_across_feature_ui, return_beeswarm_ui,
    return_feature_dependency_plot_ui, return_force_plot_ui,
    return_hit_rate_across_feature_ui, return_model_back_test_ui,
    return_most_important_feature_per_entity_ui, return_r_squared_matrix_ui)
from metis.webapp.webapp_utils import clean_strings_for_ui
from sqlalchemy.exc import ProgrammingError

logger = logging.getLogger("metis_web_app")
logger.setLevel(logging.WARNING)

try:
    load_files_from = Path.cwd().resolve()
    if (load_files_from / "models_to_explain.yaml").is_file():
        config_name = "models_to_explain.yaml"
    elif (load_files_from / "config.yaml").is_file():
        config_name = "config.yaml"
    else:
        config_name = "config.yml"

    models_to_explain = return_configuration_file(
        path=Path.cwd().resolve(),
        file_name=config_name,
    ).models_to_explain

    model_options = []
    for i, j in models_to_explain.items():
        if not j.visible_name:
            j.visible_name = " ".join(i.split("_"))
        model_options.append({"label": j.visible_name, "value": i})
except FileNotFoundError:
    model_options = {"label": "label", "value": "value"}

register_page(__name__, path="/")


@callback(
    Output("entity_choice_individual_dropdown", "options"),
    Output("entity_choice_individual_dropdown", "value"),
    Output("entity_choice_individual_dropdown", "placeholder"),
    Input("model_choice_dropdown", "value"),
)
def store_available_entity_ids(selected_model):
    if selected_model is None:
        return no_update, no_update, no_update

    available_entity_ids = return_entity_ids(
        model_config=models_to_explain[selected_model]
    )
    placeholder = f"Select an {models_to_explain[selected_model].entity_name} ID"

    if len(available_entity_ids) > 0:
        return available_entity_ids, available_entity_ids[0], placeholder
    else:
        return no_update, no_update, no_update


@callback(
    Output("feature_filter_dropdown", "options"),
    Input("model_choice_dropdown", "value"),
)
def populate_available_features(selected_model):
    if selected_model is None:
        return no_update

    avialble_attributes = return_valid_attributes_to_filter(
        models_to_explain[selected_model]
    )
    avialble_attributes = [
        {"label": i, "value": j}
        for i, j in zip(clean_strings_for_ui(avialble_attributes), avialble_attributes)
    ]

    return avialble_attributes


@callback(
    Output("feature_filter_dropdown", "value"),
    Output("feature_value_dropdown", "options"),
    Output("feature_value_dropdown", "value"),
    Output("feature_value_dropdown", "style"),
    Output("date_picker_feature_value", "min_date_allowed"),
    Output("date_picker_feature_value", "max_date_allowed"),
    Output("date_picker_feature_value", "style"),
    Input("feature_filter_dropdown", "value"),
    Input("reset_button", "n_clicks"),
    State("model_choice_dropdown", "value"),
)
def populate_available_feature_values(
    selected_feature, reset_button_clicks, selected_model
):
    if selected_feature is None:
        return no_update, no_update, "", no_update, no_update, no_update, no_update

    if ctx.triggered_id == "reset_button":
        return "", no_update, "", no_update, no_update, no_update, no_update

    valid_feature_values = return_valid_attribute_values(
        models_to_explain[selected_model], selected_feature
    )
    valid_feature_values = sorted(valid_feature_values)

    is_date_list = all(
        isinstance(value, datetime.date) for value in valid_feature_values
    )

    if is_date_list:
        return (
            no_update,
            no_update,
            no_update,
            {"display": "none"},
            min(valid_feature_values),
            max(valid_feature_values),
            {"display": "block"},
        )
    else:
        return (
            no_update,
            valid_feature_values,
            "",
            {"display": "block"},
            no_update,
            no_update,
            {"display": "none"},
        )


@callback(
    Output("shap_beeswarm_plot", "children"),
    Input("model_choice_dropdown", "value"),
    Input("apply_filters", "n_clicks"),
    Input("reset_button", "n_clicks"),
    Input("home_page_tabs", "active_tab"),
    State("feature_filter_dropdown", "value"),
    State("feature_value_dropdown", "value"),
    State("date_picker_feature_value", "start_date"),
    State("date_picker_feature_value", "end_date"),
)
def create_bee_swarm(
    selected_model,
    n_clicks,
    reset_button_clicks,
    active_tab,
    selected_feature,
    selected_feature_value,
    start_date_picker,
    end_date_picker,
):
    if selected_model not in models_to_explain:
        return no_update

    if active_tab != "audiences_tab":
        return no_update

    if ctx.triggered_id == "reset_button":
        selected_feature = None
        selected_feature_value = None
        start_date_picker = None
        end_date_picker = None

    try:
        return return_beeswarm_ui(
            model_config=models_to_explain[selected_model],
            feature=selected_feature,
            feature_value=selected_feature_value,
            start_date_picker=start_date_picker,
            end_date_picker=end_date_picker,
        )
    except (ValueError, KeyError):
        logger.exception("Failed to create beeswarm plot")
        return return_something_went_wrong_ui()


@callback(
    Output("shap_force_plot", "children"),
    Input("entity_choice_individual_dropdown", "value"),
    Input("model_choice_dropdown", "value"),
    Input("home_page_tabs", "active_tab"),
)
def create_force_plot(entity_id, selected_model, active_tab):
    if entity_id is None:
        return no_update

    if active_tab != "individual_tab":
        return no_update

    try:
        return return_force_plot_ui(
            model_config=models_to_explain[selected_model],
            selected_entity_id=entity_id,
        )
    except (ValueError, KeyError):
        logger.exception("Failed to create force plot")
        return return_something_went_wrong_ui()


@callback(
    Output("most_powerful_features", "children"),
    Input("entity_choice_individual_dropdown", "value"),
    Input("model_choice_dropdown", "value"),
    Input("home_page_tabs", "active_tab"),
)
def create_most_powerful_feature_plot_for_entities(
    entity_id,
    selected_model,
    active_tab,
):
    if entity_id is None or selected_model is None:
        return no_update

    if active_tab != "individual_tab":
        return no_update

    try:
        return return_most_important_feature_per_entity_ui(
            model_config=models_to_explain[selected_model],
            selected_entity_id=entity_id,
        )
    except (ValueError, KeyError):
        logger.exception("Failed to create plot")
        return return_something_went_wrong_ui()


@callback(
    Output("ucv_data_table", "children"),
    Input("entity_choice_individual_dropdown", "value"),
    Input("model_choice_dropdown", "value"),
    Input("home_page_tabs", "active_tab"),
)
def return_ucv_table_view_per_entity(take_id, selected_model, active_tab):
    if take_id is None:
        return no_update

    if active_tab != "individual_tab":
        return no_update

    try:
        return return_ucv_table_view_per_entity_ui(
            model_config=models_to_explain[selected_model],
            selected_entity_id=take_id,
        )
    except (ValueError, KeyError):
        logger.exception("Failed to create plot")
        return return_something_went_wrong_ui()


@callback(
    Output("hit_rate_across_feature_plot", "children"),
    Input("feature_choice_global_dropdown", "value"),
    Input("apply_filters", "n_clicks"),
    Input("reset_button", "n_clicks"),
    Input("model_choice_dropdown", "value"),
    Input("home_page_tabs", "active_tab"),
    State("feature_filter_dropdown", "value"),
    State("feature_value_dropdown", "value"),
    State("date_picker_feature_value", "start_date"),
    State("date_picker_feature_value", "end_date"),
)
def return_hit_rate_across_feature(
    feature_choice,
    n_clicks,
    reset_button_clicks,
    model_choice,
    active_tab,
    selected_feature,
    selected_feature_value,
    start_date_picker,
    end_date_picker,
):
    if feature_choice is None:
        return no_update

    if active_tab != "audiences_tab":
        return no_update

    if ctx.triggered_id == "reset_button":
        selected_feature = None
        selected_feature_value = None
        start_date_picker = None
        end_date_picker = None

    try:
        return return_hit_rate_across_feature_ui(
            model_config=models_to_explain[model_choice],
            feature=selected_feature,
            feature_value=selected_feature_value,
            global_feature=feature_choice,
            start_date_picker=start_date_picker,
            end_date_picker=end_date_picker,
        )
    except (ValueError, KeyError):
        logger.exception("Failed to create plot")
        return return_something_went_wrong_ui()


@callback(
    Output("feature_dependence_plot", "children"),
    Input("feature_one_dropdown_for_feature_dependence", "value"),
    Input("feature_two_dropdown_for_feature_dependence", "value"),
    Input("apply_filters", "n_clicks"),
    Input("reset_button", "n_clicks"),
    Input("model_choice_dropdown", "value"),
    Input("home_page_tabs", "active_tab"),
    Input("collinearity_tools_tabs", "active_tab"),
    State("feature_filter_dropdown", "value"),
    State("feature_value_dropdown", "value"),
)
def return_feature_dependance_plot(
    feature_choice_one,
    feature_choice_two,
    n_clicks,
    reset_button_clicks,
    model_choice,
    active_tab,
    collinearity_tab,
    selected_feature,
    selected_feature_value,
):
    if feature_choice_one is None or feature_choice_two is None:
        return no_update

    if active_tab != "audiences_tab" or collinearity_tab != "feature_dependence_tab":
        return no_update

    if ctx.triggered_id == "reset_button":
        selected_feature = None
        selected_feature_value = None

    if feature_choice_one == feature_choice_two:
        return (
            html.Div(
                [
                    html.Img(
                        src="assets/info.svg",
                        height="148px",
                    ),
                    html.H1("Feature Choices cannot be the same!"),
                ],
                className="info-card-container",
                style={"height": "500px"},
            ),
        )

    try:
        return return_feature_dependency_plot_ui(
            model_config=models_to_explain[model_choice],
            feature=selected_feature,
            feature_value=selected_feature_value,
            feature_one=feature_choice_one,
            feature_two=feature_choice_two,
        )
    except ValueError:
        logging.exception("Failed to create feature dependance plot")
        return return_something_went_wrong_ui()


@callback(
    Output("r_squared_matrix_plot", "children"),
    Input("model_choice_dropdown", "value"),
    Input("apply_filters", "n_clicks"),
    Input("reset_button", "n_clicks"),
    Input("home_page_tabs", "active_tab"),
    Input("collinearity_tools_tabs", "active_tab"),
    State("feature_filter_dropdown", "value"),
    State("feature_value_dropdown", "value"),
    State("date_picker_feature_value", "start_date"),
    State("date_picker_feature_value", "end_date"),
)
def return_r_squared_plot(
    model_choice,
    n_clicks,
    reset_button_clicks,
    active_tab,
    collinearity_tab,
    selected_feature,
    selected_feature_value,
    start_date_picker,
    end_date_picker,
):
    if model_choice is None:
        return no_update

    if active_tab != "audiences_tab" or collinearity_tab != "r_squared_matrix_tab":
        return no_update

    if ctx.triggered_id == "reset_button":
        selected_feature = None
        selected_feature_value = None
        start_date_picker = None
        end_date_picker = None

    try:
        return return_r_squared_matrix_ui(
            model_config=models_to_explain[model_choice],
            feature=selected_feature,
            feature_value=selected_feature_value,
            start_date_picker=start_date_picker,
            end_date_picker=end_date_picker,
        )
    except ValueError:
        logger.exception("Failed to create plot")
        return return_something_went_wrong_ui()


@callback(
    Output("vif_factor_table", "children"),
    Input("model_choice_dropdown", "value"),
    Input("home_page_tabs", "active_tab"),
    Input("collinearity_tools_tabs", "active_tab"),
)
def return_vif_table_plot(
    model_choice,
    active_tab,
    collinearity_tab,
):
    if model_choice is None:
        return no_update

    if active_tab != "audiences_tab" or collinearity_tab != "r_squared_matrix_tab":
        return no_update

    try:
        return return_variance_inflation_factor_table_ui(
            model_config=models_to_explain[model_choice],
        )
    except ValueError:
        logger.exception("Failed to create plot")
        return return_something_went_wrong_ui()


@callback(
    Output("model_validation_hit_rate_plot", "children"),
    Input("model_choice_dropdown", "value"),
    Input("home_page_tabs", "active_tab"),
)
def populate_model_validation_hit_rate_plot(
    model_choice,
    active_tab,
):
    if model_choice is None:
        return no_update

    if not models_to_explain[model_choice].model_validation_table:
        return no_update

    if active_tab != "model_performance_tab":
        return no_update

    return return_model_back_test_ui(
        model_config=models_to_explain[model_choice],
    )


@callback(
    Output("model_validation_description", "children"),
    Input("model_choice_dropdown", "value"),
)
def populate_model_validation_description(
    model_choice,
):
    if model_choice is None:
        return no_update

    return return_model_validation_description_ui()


@callback(
    Output("average_shap_value_for_feature", "children"),
    Input("feature_choice_global_dropdown", "value"),
    Input("apply_filters", "n_clicks"),
    Input("reset_button", "n_clicks"),
    Input("model_choice_dropdown", "value"),
    Input("home_page_tabs", "active_tab"),
    State("feature_filter_dropdown", "value"),
    State("feature_value_dropdown", "value"),
    State("date_picker_feature_value", "start_date"),
    State("date_picker_feature_value", "end_date"),
)
def create_segment_on_feature_plot(
    feature_choice,
    n_clicks,
    reset_button_clicks,
    model_choice,
    active_tab,
    selected_feature,
    selected_feature_value,
    start_date_picker,
    end_date_picker,
):
    if feature_choice is None:
        return no_update

    if active_tab != "audiences_tab":
        return no_update

    if ctx.triggered_id == "reset_button":
        selected_feature = None
        selected_feature_value = None
        start_date_picker = None
        end_date_picker = None

    try:
        return return_average_shap_across_feature_ui(
            model_config=models_to_explain[model_choice],
            feature=selected_feature,
            feature_value=selected_feature_value,
            global_feature=feature_choice,
            start_date_picker=start_date_picker,
            end_date_picker=end_date_picker,
        )
    except (ValueError, KeyError):
        logger.exception("Failed to create plot")
        return return_something_went_wrong_ui()


@callback(
    Output("model_description", "children"),
    Input("model_choice_dropdown", "value"),
)
def populate_model_descriptions(model_name):
    if model_name is None:
        return no_update

    return return_model_description_ui(models_to_explain[model_name])


@callback(
    Output("model_explained_factor", "children"),
    Input("model_choice_dropdown", "value"),
)
def populate_model_explained_factor(model_name):
    if model_name is None:
        return no_update
    try:
        return_ui = return_model_explained_factor_ui(models_to_explain[model_name])
    except (ProgrammingError, IndexError):
        return_ui = html.Div()
        logger.exception("Failed to create model explained factor plot")

    return return_ui


@callback(
    Output("feature_choice_global_dropdown", "options"),
    Output("feature_one_dropdown_for_feature_dependence", "options"),
    Output("feature_two_dropdown_for_feature_dependence", "options"),
    Output("feature_choice_global_dropdown", "value"),
    Output("feature_one_dropdown_for_feature_dependence", "value"),
    Output("feature_two_dropdown_for_feature_dependence", "value"),
    Input("model_choice_dropdown", "value"),
    State("feature_choice_global_dropdown", "value"),
    State("feature_one_dropdown_for_feature_dependence", "value"),
    State("feature_two_dropdown_for_feature_dependence", "value"),
)
def update_available_features(
    selected_model, selected_feature_state, feature_one_state, feature_two_state
):
    """
    Update the available features for the global dropdown based on the data.

    Returns:
        list: The options for the "feature_choice_global_dropdown" output.
    """
    if selected_model is None:
        return [no_update] * 6

    options_list = return_valid_fields_to_explain_with()

    options = [
        {"label": i, "value": j}
        for i, j in zip(clean_strings_for_ui(options_list), options_list)
    ]
    options = sorted(options, key=lambda x: x["label"])

    # maintains the values in the dropdowns between model changes.
    if not all(
        state is not None and state in options_list
        for state in [selected_feature_state, feature_one_state, feature_two_state]
    ):
        return (
            options,
            options,
            options,
            options[0]["value"],
            options[0]["value"],
            options[1]["value"],
        )
    else:
        return (
            options,
            options,
            options,
            selected_feature_state,
            feature_one_state,
            feature_two_state,
        )


@callback(
    Output("entity_id_text", "children"),
    Input("model_choice_dropdown", "value"),
)
def update_title_string_with_entity_names(selected_model):
    if selected_model is None:
        return no_update
    return f"{models_to_explain[selected_model].entity_name} ID:"


@callback(
    Output("model_performance_tab", "disabled"),
    Output("model_performance_tab", "label_style"),
    Output("model_performance_tab_tooltip", "style"),
    Input("model_choice_dropdown", "value"),
)
def check_if_model_performance_tab_should_show(selected_model):

    if selected_model is None:
        return no_update, {"display": "none"}, {"display": "none"}

    if models_to_explain[selected_model].ui_config.show_model_performance_page is False:
        return True, {"display": "none"}, {"display": "none"}

    if models_to_explain[selected_model].model_validation_table:
        return False, {"display": "block"}, {"display": "none"}

    return True, {"display": "block"}, {"display": "block"}


@callback(
    Output("individual_tab", "label_style"),
    Input("model_choice_dropdown", "value"),
)
def check_if_model_individual_tab_should_show(selected_model):

    if selected_model is None:
        return {"display": "none"}

    if models_to_explain[selected_model].ui_config.show_individual_page is False:
        return {"display": "none"}

    return {"display": "block"}


@callback(
    Output("audiences_tab", "label_style"),
    Input("model_choice_dropdown", "value"),
)
def check_if_model_audiences_tab_should_show(selected_model):

    if selected_model is None:
        return {"display": "none"}

    return {"display": "block"}


@callback(
    Output("the_sages_wisdom", "is_open"),
    Output("the_sages_advice_text", "children"),
    Input("open_sages_wisdom_beeswarm", "n_clicks"),
    Input("the_sages_wisdom-close-modal", "n_clicks"),
    State("model_choice_dropdown", "value"),
    running=[Output("explaination-loading", "is_open"), True, False],
    prevent_initial_call=True,
)
def toggle_modal(n1, n2, selected_model):
    if selected_model is None:
        return no_update, no_update

    button_clicked = ctx.triggered_id

    if "-close-modal" in button_clicked:
        return False, ""

    if "beeswarm" in button_clicked:
        ui_element_clicked = "beeswarm"
    else:
        ui_element_clicked = "not_yet_implemented"

    if "open_sages_wisdom" in button_clicked:
        try:
            wisdom = return_hermes_sage_advice(
                ui_element_clicked, models_to_explain[selected_model]
            )
        except Exception:
            wisdom = "The sages wisdom is not available at this time."

        return True, wisdom

    return no_update, no_update


@callback(
    Output("open_sages_wisdom_beeswarm", "style"),
    Input("model_choice_dropdown", "value"),
)
def show_sages_wisdom_buttons(selected_model):
    if selected_model is None:
        return no_update

    run_llm_from_settings = models_to_explain[selected_model].llm_settings.llm_run_llm
    if run_llm_from_settings and check_for_api_key():
        return {"display": "block"}
    else:
        return {"display": "None"}


# ----------------------------- #
# Callbacks that run clientside #
# ----------------------------- #

clientside_callback(
    """
    function swap_around_uis(selected_model) {
        if (selected_model !== null && selected_model !== undefined) {
            output = [
                "show-me-after-first-load",
                "show-me-after-first-load",
                "show-me-after-first-load",
                "hide-me-until-first-load",
                "hide-me-until-first-load",
                "hide-me-until-first-load"
            ];
        } else {
            output = [
                "hide-me-until-first-load",
                "hide-me-until-first-load",
                "hide-me-until-first-load",
                "show-me-after-first-load",
                "show-me-after-first-load",
                "show-me-after-first-load"
            ];
        }

        return output;
    }
""",
    Output("audiences_tab_content", "className"),
    Output("individual_tab_content", "className"),
    Output("model_performance_tab_content", "className"),
    Output("pre_load_audiences_tab_content", "className"),
    Output("pre_load_individual_tab_content", "className"),
    Output("pre_load_model_performance_tab_content", "className"),
    Input("model_choice_dropdown", "value"),
)

clientside_callback(
    """
    function(n1, IsOpen) {
        if (n1 !== undefined && n1 !== 0) {
            return !IsOpen;
        }
    }
    """,
    Output("offcanvas-scrollable", "is_open"),
    Input("open-offcanvas-scrollable", "n_clicks"),
    State("offcanvas-scrollable", "is_open"),
)

scrollable_canvas = return_glossary_ui()

header = dbc.Row(
    [
        dbc.Col(
            [
                dbc.Col(
                    [
                        html.H1("Model Explainability", style={"marginBottom": "0px"}),
                        html.P(
                            "Understand what drives your model outputs.",
                            style={"marginBottom": "4px"},
                        ),
                    ],
                ),
                html.Div(
                    [
                        html.Div(
                            [
                                html.H5("Selected Model: ", id="model_selection_text"),
                                dcc.Dropdown(
                                    options=model_options,
                                    value=None,
                                    id="model_choice_dropdown",
                                    className="select-dropdown",
                                    placeholder="Select a Model",
                                    clearable=False,
                                ),
                            ],
                            className="gloabl-settings",
                        ),
                        scrollable_canvas,
                    ],
                    style={
                        "display": "grid",
                        "gridAutoFlow": "column",
                    },
                ),
            ],
            className="title-container",
        ),
    ],
)

audiences_tab = html.Div(
    [
        html.Div(
            [
                html.Div(
                    [
                        html.Img(
                            src="assets/info.svg",
                            height="148px",
                        ),
                        html.H1("Please choose a model first!"),
                    ],
                    className="info-card-container",
                ),
            ],
            id="pre_load_audiences_tab_content",
            className="show-me-after-first-load",
        ),
        html.Div(
            [
                html.Div(
                    [
                        html.Div(
                            [
                                html.Div(id="model_description"),
                                html.Div(id="model_explained_factor"),
                                html.Div(
                                    [
                                        html.H2("Filters"),
                                        html.H5("Attribute"),
                                        dcc.Dropdown(
                                            options=[],
                                            value=None,
                                            id="feature_filter_dropdown",
                                            className="select-dropdown",
                                            placeholder="Select a feature to filter",
                                        ),
                                        html.H5(
                                            "Attribute Value",
                                            style={"marginTop": "8px"},
                                        ),
                                        dcc.Dropdown(
                                            options=[],
                                            value=None,
                                            id="feature_value_dropdown",
                                            className="select-dropdown",
                                            placeholder="Select a feature value to filter",
                                        ),
                                        dcc.DatePickerRange(
                                            id="date_picker_feature_value",
                                            min_date_allowed=None,
                                            max_date_allowed=None,
                                            style={"display": "none"},
                                        ),
                                        html.Div(
                                            [
                                                dbc.Button(
                                                    "Reset",
                                                    id="reset_button",
                                                    className="button_white",
                                                    n_clicks=0,
                                                    style={},
                                                ),
                                                dbc.Button(
                                                    "Apply",
                                                    id="apply_filters",
                                                    className="button-blue-small",
                                                    n_clicks=0,
                                                    style={},
                                                ),
                                            ],
                                            style={
                                                "display": "flex",
                                                "marginTop": "8px",
                                            },
                                        ),
                                    ],
                                ),
                            ],
                        ),
                        html.Div(
                            [
                                dcc.Loading(
                                    html.Div(
                                        id="shap_beeswarm_plot",
                                        style={"height": "500px"},
                                    ),
                                ),
                                dbc.Button(
                                    "LLM",
                                    id="open_sages_wisdom_beeswarm",
                                    n_clicks=0,
                                    className="button-blue-small",
                                    style={"display": "none"},
                                ),
                            ],
                        ),
                    ],
                    className="top-row-audiences",
                ),
                html.Div(
                    [
                        html.Div(
                            [
                                html.H4("Feature based views"),
                                html.H5(
                                    "Select a Feature: ",
                                    id="model_selection_text",
                                ),
                                dcc.Dropdown(
                                    options=[""],
                                    value=None,
                                    id="feature_choice_global_dropdown",
                                    className="select-dropdown",
                                    placeholder="Select a Feature",
                                    disabled=False,
                                    clearable=False,
                                ),
                            ],
                            className="audiences-segment-settings",
                        ),
                        html.Div(
                            [
                                dcc.Loading(
                                    html.Div(
                                        id="average_shap_value_for_feature",
                                        className="second-row-audiences-tile",
                                    ),
                                ),
                                dcc.Loading(
                                    html.Div(
                                        id="hit_rate_across_feature_plot",
                                        className="second-row-audiences-tile",
                                    ),
                                ),
                            ],
                            className="second-row-audiences",
                        ),
                    ],
                    className="bottom-row-audiences",
                ),
                dbc.Accordion(
                    [
                        dbc.AccordionItem(
                            [
                                html.Div(
                                    [
                                        dbc.Tabs(
                                            [
                                                dbc.Tab(
                                                    html.Div(
                                                        [
                                                            html.Div(
                                                                [
                                                                    html.H3(
                                                                        "Correlation Matrix:"
                                                                    ),
                                                                    dcc.Loading(
                                                                        html.Div(
                                                                            id="r_squared_matrix_plot",
                                                                        ),
                                                                    ),
                                                                ],
                                                                style={
                                                                    "margin": "8px",
                                                                    "overflow": "auto",
                                                                },
                                                            ),
                                                            html.Div(
                                                                [
                                                                    html.H3(
                                                                        "Variance Inflation Factor:"
                                                                    ),
                                                                    dcc.Loading(
                                                                        html.Div(
                                                                            id="vif_factor_table",
                                                                        ),
                                                                    ),
                                                                ],
                                                                style={
                                                                    "margin": "8px",
                                                                    "overflow": "auto",
                                                                },
                                                            ),
                                                        ],
                                                        className="correlation-tab-inner-div",
                                                        style={"height": "80vh"},
                                                    ),
                                                    label="Correlation Analysis",
                                                    id="r_squared_matrix_tab",
                                                    tab_id="r_squared_matrix_tab",
                                                    className="main-tab",
                                                    label_style={
                                                        "width": "fit-content"
                                                    },
                                                ),
                                                dbc.Tab(
                                                    html.Div(
                                                        [
                                                            html.Div(
                                                                [
                                                                    html.H5(
                                                                        "Select Primary Feature: ",
                                                                        style={
                                                                            "marginRight": "8px",
                                                                            "marginTop": "4px",
                                                                        },
                                                                    ),
                                                                    dcc.Dropdown(
                                                                        options=[""],
                                                                        value=None,
                                                                        id="feature_one_dropdown_for_feature_dependence",
                                                                        className="select-dropdown",
                                                                        placeholder="Select a Feature",
                                                                        disabled=False,
                                                                        clearable=False,
                                                                        style={
                                                                            "marginRight": "8px"
                                                                        },
                                                                    ),
                                                                    html.H5(
                                                                        "Select Secondary Feature: ",
                                                                        style={
                                                                            "marginRight": "8px",
                                                                            "marginTop": "4px",
                                                                        },
                                                                    ),
                                                                    dcc.Dropdown(
                                                                        options=[""],
                                                                        value=None,
                                                                        id="feature_two_dropdown_for_feature_dependence",
                                                                        className="select-dropdown",
                                                                        placeholder="Select a Feature",
                                                                        disabled=False,
                                                                        clearable=False,
                                                                    ),
                                                                ],
                                                                style={
                                                                    "margin": "8px",
                                                                    "display": "flex",
                                                                    "paddingBottom": "8px",
                                                                },
                                                            ),
                                                            html.Div(
                                                                id="feature_dependence_plot"
                                                            ),
                                                        ],
                                                        style={"height": "80vh"},
                                                    ),
                                                    label="Feature Dependence",
                                                    id="feature_dependence_tab",
                                                    tab_id="feature_dependence_tab",
                                                    className="main-tab",
                                                    label_style={
                                                        "width": "fit-content"
                                                    },
                                                ),
                                            ],
                                            id="collinearity_tools_tabs",
                                            active_tab="r_squared_matrix_tab",
                                        ),
                                    ],
                                    className="bottom-row-audiences",
                                ),
                            ],
                            title="Colinearity Tools:",
                        ),
                    ],
                    start_collapsed=True,
                    style={"padding": "8px"},
                ),
            ],
            id="audiences_tab_content",
            className="hide-me-until-first-load",
        ),
    ],
)

individual_tab = html.Div(
    [
        html.Div(
            [
                html.Div(
                    [
                        html.Img(
                            src="assets/info.svg",
                            height="148px",
                        ),
                        html.H1("Please choose a model first!"),
                    ],
                    className="info-card-container",
                ),
            ],
            id="pre_load_individual_tab_content",
            className="show-me-after-first-load",
        ),
        html.Div(
            [
                html.Div(
                    [
                        html.Div(
                            [
                                html.H3(
                                    "Entity Id:",
                                    id="entity_id_text",
                                    style={"margin": "8px"},
                                ),
                                dcc.Dropdown(
                                    options=[],
                                    value=None,
                                    id="entity_choice_individual_dropdown",
                                    className="select-dropdown",
                                    placeholder="Select an Entity ID",
                                    clearable=False,
                                ),
                            ],
                            style={"display": "flex", "marginTop": "8px"},
                        ),
                    ],
                    className="top-row-individual",
                ),
                html.Div(
                    [
                        dcc.Loading(
                            html.Div(
                                id="most_powerful_features",
                                className="second-row-individual-tile",
                            ),
                        ),
                        dcc.Loading(
                            html.Div(
                                id="ucv_data_table",
                                className="second-row-individual-tile",
                            ),
                        ),
                    ],
                    className="second-row-individual",
                ),
                html.Div(
                    [
                        html.Div(
                            id="shap_force_plot",
                            className="second-row-individual-tile",
                        ),
                    ],
                    className="top-row-individual",
                ),
            ],
            id="individual_tab_content",
            className="hide-me-until-first-load",
        ),
    ],
)

model_performance_tab = html.Div(
    [
        html.Div(
            [
                html.Div(
                    [
                        html.Img(
                            src="assets/info.svg",
                            height="148px",
                        ),
                        html.H1("Please choose a model first!"),
                    ],
                    className="info-card-container",
                ),
            ],
            id="pre_load_model_performance_tab_content",
            className="show-me-after-first-load",
        ),
        html.Div(
            [
                html.Div(
                    [
                        html.Div(id="model_validation_description"),
                        dcc.Loading(
                            html.Div(
                                id="model_validation_hit_rate_plot",
                                style={"height": "500px"},
                            ),
                        ),
                    ],
                    className="top-row-model-performance",
                ),
            ],
            id="model_performance_tab_content",
            className="hide-me-until-first-load",
        ),
    ]
)

tabs = html.Div(
    [
        dbc.Tabs(
            [
                dbc.Tab(
                    audiences_tab,
                    label="Audiences",
                    id="audiences_tab",
                    tab_id="audiences_tab",
                    className="main-tab",
                    label_style={
                        "display": "none",
                    },
                ),
                dbc.Tab(
                    individual_tab,
                    label="Individual",
                    id="individual_tab",
                    tab_id="individual_tab",
                    className="main-tab",
                    label_style={
                        "display": "none",
                    },
                ),
                dbc.Tab(
                    model_performance_tab,
                    label="Model Performance",
                    id="model_performance_tab",
                    tab_id="model_performance_tab",
                    className="main-tab",
                    label_style={
                        "display": "none",
                    },
                ),
            ],
            id="home_page_tabs",
            active_tab="audiences_tab",
        ),
    ],
)

llm_output_loading_modal = return_modal(
    id="explaination-loading",
    body=html.Div(
        children=[
            html.Div(className="dash-spinner"),
        ]
    ),
    footer="",
    size="lg",
    show_close=False,
    className="explain-optimization-loader",
)

llm_output_window_modal = return_modal(
    id="the_sages_wisdom",
    body=[
        dcc.Loading(
            html.Div(
                id="the_sages_advice_text",
            ),
        )
    ],
    title=html.Img(src="explain_icon.svg"),
    detail="",
    footer=[html.Div()],
    footer_align="right",
    size="lg",
    show_back=False,
    show_close=True,
)

layout = [
    header,
    tabs,
    llm_output_window_modal,
    llm_output_loading_modal,
    dbc.Tooltip(
        "No validation data available for this model.",
        target="model_performance_tab",
        placement="top",
        id="model_performance_tab_tooltip",
    ),
]
