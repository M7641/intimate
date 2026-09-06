import logging
import os
import shutil
import socket
from pathlib import Path

import typer
import yaml
from metis.configuration.config_files import return_configuration_file
from metis.utility_tools.connect import drop_table
from metis.workflow_bootstrap import bootstrap_workflow
from metis.workflows.create_shap_values import create_shap_values
from metis.workflows.train_explain_models import train_models_to_explain
from sqlalchemy.exc import ProgrammingError
from typing_extensions import Annotated

from .press_deploy import (deploy_model_explain_app,
                           deploy_model_explain_blocks, update_app_spec,
                           update_webapp_spec, update_workflow_spec)

os.environ["TF_CPP_MIN_LOG_LEVEL"] = "3"

logging.getLogger("tensorflow").disabled = True
app = typer.Typer()

tenants_shared_to = [
    "CENSORED",
]


def run_clean_up():
    file_system_back_end_check = Path.cwd().resolve() / "file_system_backend"
    if file_system_back_end_check.is_dir():
        shutil.rmtree(str(file_system_back_end_check))


def build_dockerfile(dockerfile_name):
    pick_up_dockerfile = Path(__file__).resolve().parents[1] / dockerfile_name
    send_to_location = Path().cwd() / dockerfile_name
    shutil.copyfile(str(pick_up_dockerfile), str(send_to_location))


def check_config_file(return_config_file_name=False):
    load_files_from = Path.cwd().resolve()

    possible_config_names = [
        "models_to_explain.yaml",
        "models_to_explain.yml",
        "config.yaml",
        "config.yml",
    ]
    for i in possible_config_names:
        if (load_files_from / i).is_file():
            config_name = i
            break
    else:
        config_name = "models_to_explain.yaml"

    if return_config_file_name:
        return config_name


def check_for_glossary():
    load_files_from = Path.cwd().resolve()
    if not (load_files_from / "glossary.yaml").is_file():
        msg = (
            "No glossary present!"
            + " please run from dir with glossary.yaml file if desired!"
        )
        logging.warning(msg)


@app.command()
def clean_db(schema: Annotated[str, "Schema to clean"] = typer.Option("TRANSFORM")):

    tables_to_drop = [
        "metis_explaining_model_validation",
        "metis_model_explainability_output_predictions",
        "metis_shap_values_for_model_explain",
        "metis_data_used_to_evaluate_shap",
        "metis_shap_base_rates",
    ]

    user_response = typer.confirm(
        f"Are you sure you want to drop {', '.join(tables_to_drop)} from Schema: {schema}?",
        abort=True,
    )

    if not user_response:
        raise typer.Abort()

    for table in tables_to_drop:
        try:
            drop_table(schema, table)
            typer.echo(f"Dropped {table} from {schema}")
        except ProgrammingError:
            typer.echo(f"Failed to drop {table} from {schema}")


@app.command()
def generate_config():
    """
    Generates a configuration file named 'models_to_explain.yaml'.
    """
    sample_config = {
        "models_to_explain": {
            "model_name": {
                "visible_name": "Model Name",
                "table_used_to_explain": "schema.table",
                "response_variable_table_name": "schema.table",
                "response_variable_field_name": "response_variable",
                "fields_to_explain_with": ["*"],
                "model_type": "binary_classifier",
                "entity_id": "customer_id",
                "entity_name": "Customer",
                "model_type_to_use": "lightgbm",
            }
        }
    }
    path_to_save = Path.cwd().resolve() / "models_to_explain.yaml"
    with path_to_save.open(encoding="utf-8", mode="w") as file:
        yaml.dump(sample_config, file)


@app.command()
def build_workflow():

    load_files_from = Path.cwd().resolve()

    if not (load_files_from / "Dockerfile.workflow").is_file():
        build_dockerfile("Dockerfile.workflow")

    models_to_explain = return_configuration_file(
        path=str(load_files_from),
        file_name=check_config_file(return_config_file_name=True),
    ).models_to_explain
    bootstrap_workflow(models_to_explain)


@app.command()
def run_workflow(
    run_prod: Annotated[bool, "Run in production mode."] = typer.Option(False),
    model_name: Annotated[
        str, "Name of the model to run the workflow for."
    ] = typer.Option("all"),
) -> None:
    """
    Run the workflow for model explanation.

    Args:
        run_prod (bool, optional): Flag indicating whether to run in production mode. Defaults to False.
        model_name (str, optional): Name of the model to run the workflow for. Defaults to "all".
    """
    load_files_from = Path.cwd().resolve()
    logging.info(f"Runing from '{load_files_from}'")

    os.environ["OUTPUT_DB_SCHEMA"] = "PUBLISH" if run_prod else "TRANSFORM"

    models_to_explain = return_configuration_file(
        path=str(load_files_from),
        file_name=check_config_file(return_config_file_name=True),
    ).models_to_explain

    if model_name == "all":
        for model_name, config in models_to_explain.items():
            train_models_to_explain(config)
            create_shap_values(run_prod, config)
    else:
        model_config = models_to_explain[model_name]
        model_config.name = model_name
        train_models_to_explain(model_config)
        create_shap_values(run_prod, model_config)


@app.command()
def run_app(run_prod: bool = typer.Option(False)):
    """
    Run the Model Explainability web application.

    Args:
        run_prod (bool, optional): Flag indicating whether to run the application
            in production mode. Defaults to False.
    """
    from metis.webapp.app import web_app  # noqa: F401

    os.environ["OUTPUT_DB_SCHEMA"] = "PUBLISH" if run_prod else "TRANSFORM"

    if run_prod:
        web_app.run_server(
            port=8050,
            host="0.0.0.0",
            debug=False,
        )
    else:
        hostname = socket.gethostname()
        if "workspace" in hostname:
            workspace_id = hostname.split("workspace-")[1][:-2]
            user_name = os.getenv("USER")  # pwd.getpwuid(os.getuid())[0]
        typer.echo(
            typer.style(
                f"https://{os.getenv('TENANT')}-{workspace_id}"
                + f".nimbus.example/user/{user_name}/proxy/8050/",
                fg=typer.colors.GREEN,
            )
        )
        web_app.run_server(
            port=8050,
            host="127.0.0.1",
            debug=True,
            use_reloader=False,
        )


@app.command()
def pressify() -> None:
    deploy_model_explain_blocks(tenants_shared_to)
    deploy_model_explain_app(tenants_shared_to)
    return None


@app.command()
def update_press(
    major: Annotated[bool, "Update Press block to major version"] = typer.Option(False),
    minor: Annotated[bool, "Update Press block to minor version"] = typer.Option(False),
) -> None:

    semantic_level = "major" if major else "minor" if minor else "patch"

    _ = update_workflow_spec(semantic_level, tenants_shared_to)
    _ = update_webapp_spec(semantic_level, tenants_shared_to)
    update_app_spec(
        semantic_level=semantic_level,
        tenants_shared_to=tenants_shared_to,
    )
    return None


if __name__ == "__main__":
    app()
