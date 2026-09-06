import os
from pathlib import Path

import dash
from jinja2 import Template
from metis.configuration.configuration_classes import ExplainModel
from metis.utility_tools.connect import load_dataframe_from_string_query
from openai import OpenAI, OpenAIError

try:
    client = OpenAI(api_key=os.getenv("OPENAI_API_KEY"))
except OpenAIError:
    client = None


def check_for_api_key() -> bool:
    if os.getenv("OPENAI_API_KEY", None) is None:
        return False
    return True


def get_hermes_shap_metrics() -> str:
    build_query = """
        SELECT
            FEATURE,
            AVG(ABS(SHAP_VALUE::FLOAT)) AS AVG_SHAPABS
        FROM PUBLISH.DATA_USED_TO_EVALUATE_SHAP
        LEFT JOIN PUBLISH.SHAP_VALUES_FOR_MODEL_EXPLAIN
            USING(FEATURE, CUSTOMER_ID)
        GROUP BY FEATURE
    """
    return load_dataframe_from_string_query(
        query_str=build_query,
        try_optimise_memory_useage=True,
    ).to_json(index=False)


def get_beeswarm_promt(model_config: ExplainModel) -> str:
    """
    Amusing custom promt additions as a toggle.

    Thinking Jinja to template it from a promt file to save this
    python script getting too long.
    """

    shap_data = get_hermes_shap_metrics()

    file_location = (
        Path(__file__).parent.resolve() / "promt_library/beeswarm_promt.jinja"
    )
    with file_location.open(encoding="utf-8") as file:
        promt_template = file.read()

    llm_set = model_config.llm_settings

    beeswaem_promt = Template(promt_template)
    beeswaem_promt_rendered = beeswaem_promt.render(
        llm_response_variable=llm_set.llm_response_variable,
        llm_input_description=llm_set.llm_input_description,
        llm_response_variable_description=llm_set.llm_response_variable_description,
        shap_data=shap_data,
    )
    return beeswaem_promt_rendered


def get_hermes_response(ui_element: str, model_config: ExplainModel) -> str:
    ui_elements = {
        "beeswarm": get_beeswarm_promt(model_config),
        "not_yet_implemented": "",
    }

    if ui_elements.get(ui_element):
        chat_completion = client.chat.completions.create(
            messages=[
                {
                    "role": "user",
                    "content": ui_elements.get(ui_element),
                }
            ],
            model="gpt-3.5-turbo",
            temperature=1.5,
        )
    else:
        return "Large Language Model is currently sleeping, please try again later."

    return chat_completion.choices[0].message.content


def return_hermes_sage_advice(ui_element: str, model_config: ExplainModel) -> str:

    the_sage_advice = get_hermes_response(ui_element, model_config)

    return dash.html.Div([dash.html.Span(the_sage_advice)])
