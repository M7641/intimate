import logging
import os
import shutil
from datetime import datetime
from pathlib import Path

import joblib
import numpy as np
import pandas as pd
import pytz
from metis.configuration.configuration_classes import ExplainModel
from metis.utility_tools.connect import (drop_model_data_from_table,
                                         upload_data_to_table)
from metis.utility_tools.s3_tools import download_most_recent_tar_file
from metis.utility_tools.shap_tools import MetisSHAP
from metis.workflows.workflow_fetch_data import return_data_for_explain

root_path = Path(__file__).parent.resolve()


def create_shap_values(run_prod: bool, model_config: ExplainModel) -> None:
    logging.info("%s - Starting!", model_config.name)

    logging.info("%s - Deleting old data!", model_config.name)

    for table in [
        "metis_shap_values_for_model_explain",
        "metis_data_used_to_evaluate_shap",
        "metis_shap_base_rates",
    ]:
        drop_model_data_from_table(
            os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"), table, model_config.name
        )

    save_directory = root_path / model_config.name
    save_directory.mkdir(parents=True, exist_ok=True)

    s3_key = f"{os.environ.get('TENANT')}/datascience/models/{model_config.name}/explainability_models/"

    download_most_recent_tar_file(
        save_dir=str(save_directory),
        s3_key=s3_key,
    )
    model = joblib.load(str(save_directory) + "/temp/explainability_models.joblib")
    if Path(save_directory).is_dir():
        shutil.rmtree(save_directory)

    data_to_explain_with = return_data_for_explain(model_config)

    take_fraction = 1.0 if run_prod else 0.05
    take_sampled_data = data_to_explain_with.set_index("table_id")

    if len(take_sampled_data) > model_config.max_number_of_rows_to_explain:
        take_sampled_data = take_sampled_data.sample(
            model_config.max_number_of_rows_to_explain
        )

    take_sampled_data = take_sampled_data.sample(frac=take_fraction)

    shap_values = MetisSHAP(
        model=model,
        model_config=model_config,
        data_model_was_trained_on=take_sampled_data,
    )

    if model_config.model_type_to_use == "neural_network":
        expliner_type = "DeepExplainer"
    elif model_config.model_type_to_use == "lightgbm":
        expliner_type = "TreeExplainer"

    shap_values.init(explainer_type=expliner_type)

    logging.info(" %s -  Explainability Model Made!", model_config.name)

    shap_values_dataframe = (
        shap_values.get_shap_values_as_dataframe()
        .reset_index()
        .rename(columns={"index": "table_id"})
    )
    shap_values_dataframe = pd.melt(
        shap_values_dataframe.reset_index(drop=True),
        id_vars="table_id",
        var_name="feature",
        value_name="shap_value",
    )
    shap_values_dataframe["model_name"] = model_config.name
    upload_data_to_table(
        os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"),
        "metis_shap_values_for_model_explain",
        shap_values_dataframe,
    )

    data_model_was_trained_on = (
        shap_values.get_data_model_was_trained_on_data().reset_index()
    )
    data_model_was_trained_on = data_model_was_trained_on.replace({np.nan: None})
    data_model_was_trained_on = pd.melt(
        data_model_was_trained_on.reset_index(drop=True),
        id_vars="table_id",
        var_name="feature",
        value_name="value",
    )
    data_model_was_trained_on["model_name"] = model_config.name
    upload_data_to_table(
        os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"),
        "metis_data_used_to_evaluate_shap",
        data_model_was_trained_on,
    )

    data_dict = {
        "model_name": model_config.name,
        "shap_base_rate": shap_values.get_base_value_for_explainer(),
        "updated_at": datetime.now(pytz.utc).strftime("%Y-%m-%d %H:%M:%S"),
    }
    base_rates = pd.DataFrame(data=data_dict, index=[0])
    upload_data_to_table(
        os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"), "metis_shap_base_rates", base_rates
    )
