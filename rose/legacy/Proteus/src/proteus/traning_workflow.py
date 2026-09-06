import logging
import shutil
from pathlib import Path
from typing import Literal

from proteus.build_train_test_validation_split import \
    build_train_test_validation_split
from proteus.config import load_config
from proteus.workflow_steps.create_final_model_output_step import \
    create_final_model_outputs
from proteus.workflow_steps.train_model_step import train_model
from proteus.workflow_steps.validate_model_step import validate_model

logging.basicConfig(
    filename="model_run.log",
    level=logging.INFO,
    filemode="w",
    format="%(name)s - %(levelname)s - %(message)s",
)


def model_train_workflow(
    model_name: str,
    dev_test_prod_setting: str,
    config_dir: Path,
    re_train_model: Literal["train", "keep-previous"],
    root_path: Path,
) -> None:
    """
    Todo:
    ----
    1. Better erroring when there are non unique Ids passed as the entity ID.
    """
    logging.info("Starting workflow for model: %model_name", model_name)
    config = load_config(config_dir)
    if re_train_model == "train":
        build_train_test_validation_split(
            model_name=model_name,
            config=config,
            dev_test_prod_setting=dev_test_prod_setting,
            root_path=root_path,
        )

        train_model(
            model_name,
            config,
            dev_test_prod_setting=dev_test_prod_setting,
            root_path=root_path,
        )

        try:
            validate_model(
                model_name,
                config,
                dev_test_prod_setting=dev_test_prod_setting,
                root_path=root_path,
            )
        except ValueError:
            logging.exception(
                "Validation failed for model: %model_name",
                model_name,
            )
            pass

    create_final_model_outputs(
        model_name,
        config,
        dev_test_prod_setting=dev_test_prod_setting,
        root_path=root_path,
    )

    delete_dir = root_path / "data/"
    if delete_dir.is_dir():
        shutil.rmtree(str(delete_dir))
    logging.info("Success for model: %model_name", model_name)
