import logging
from pathlib import Path

from metis.configuration.configuration_classes import ExplainModel
from metis.model_training import train_meta_model
from metis.workflows.workflow_fetch_data import (
    return_data_for_explain, return_model_target_for_explain)

root_path = Path(__file__).parent.resolve()


def train_models_to_explain(model_config: ExplainModel) -> None:

    logging.info("%s - Starting!", model_config.name)

    data_to_explain_with = return_data_for_explain(model_config)
    target = return_model_target_for_explain(model_config)

    train_meta_model(
        data_to_explain_with,
        target,
        model_config=model_config,
    )

    logging.info(" %s -  Explainability Model Made!", model_config.name)
