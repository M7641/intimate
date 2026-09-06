"""Functions to validate models trained by this package."""
import logging
import shutil
from pathlib import Path

import numpy as np
import pandas as pd
from proteus.config.pydantic_models import ConfigFile, ModelConfigSingular
from proteus.connect import (DBConnector, create_engine,
                             load_dataframe_from_string_query)
from proteus.dev_test_prod_utils import get_model_save_name, get_schema
from proteus.model_tools.data_processing import (build_model_input,
                                                 load_data_from_parquets)
from proteus.model_tools.model_utils import build_model_constructor_config
from proteus.model_tools.s3_tools import download_most_recent_tar_file
from proteus.workflow_steps.create_data_step import build_model_ready_data
from sqlalchemy.orm import Session

# Everything breaks when this is imported earlier, no idea why.
import tensorflow as tf  # isort: skip


def validate_binary_classifier(
    model_name: str,
    output: pd.DataFrame,
    tf_model: tf.keras.Model,
    model_ready_data: dict,
    model_config: ModelConfigSingular,
) -> pd.DataFrame:
    """Validates a binary classifier.

    Args:
    ----
        model_name (str): The name of the model.
        output (pd.DataFrame): The DataFrame containing the validation
            data and predictions.
        tf_model (tf.keras.Model): The trained binary classifier model.
        model_ready_data (dict): The dictionary containing the validation
            data in the required format.

    Returns:
    -------
        pd.DataFrame: The DataFrame with additional columns for model
            predictions and performance metrics.
    """
    output["model_prediction"] = tf_model.predict(
        model_ready_data["validation_split"]["validation_array"],
        verbose=2,
    ).reshape(-1)

    try:
        output["score"] = pd.qcut(
            output["model_prediction"],
            q=[0.0, 0.5, 0.75, 0.85, 0.95, 1.0],
            labels=[
                "1 - Very Low",
                "2 - Low",
                "3 - Medium",
                "4 - High",
                "5 - Very High",
            ],
            duplicates="drop",
        )
    except ValueError:
        output["score"] = "2 - Low"

    output["predicted"] = output["model_prediction"] > 0.5
    output["predicted"] = output["predicted"].astype(int)

    score_perf = output.groupby("score").agg(
        {model_config.response_variable_attribute_name: ["sum", "mean", np.size]},
    )

    confusion_matrix = tf.math.confusion_matrix(
        np.asarray(output[model_config.response_variable_attribute_name]).astype(
            np.float32,
        ),
        np.asarray(output["predicted"]).astype(np.float32),
    )

    msg = f"\n \n Model: {model_name} Validation Start \n \n"
    logging.info(msg)
    logging.info(score_perf)
    logging.info(confusion_matrix)
    msg2 = f"\n \n Model: {model_name} Validation End \n \n"
    logging.info(msg2)

    return output


def validate_multi_classifier(
    model_name: str,
    output: pd.DataFrame,
    tf_model: tf.keras.Model,
    model_ready_data: dict,
    model_config: ModelConfigSingular,
) -> pd.DataFrame:
    """Validate a multi-classifier model.

    Args:
    ----
        model_name (str): The name of the model.
        output (pd.DataFrame): The DataFrame containing the target
            and model predictions.
        tf_model (tf.keras.Model): The trained TensorFlow model.
        model_ready_data (dict): The dictionary containing the
            validation data.

    Returns:
    -------
        pd.DataFrame: The DataFrame with the updated model predictions.

    """
    output["model_prediction"] = np.argmax(
        tf_model.predict(
            model_ready_data["validation_split"]["validation_array"],
            verbose=2,
        ),
        axis=1,
    )

    confusion_matrix = tf.math.confusion_matrix(
        np.asarray(output[model_config.response_variable_attribute_name]).astype(
            np.float32,
        ),
        np.asarray(output["model_prediction"]).astype(np.float32),
    )
    msg = f"\n \n Model: {model_name} Validation Start \n \n"
    logging.info(msg)
    logging.info(confusion_matrix)
    msg2 = f"\n \n Model: {model_name} Validation End \n \n"
    logging.info(msg2)
    return output


def validate_regression_model(
    output: pd.DataFrame,
    tf_model: tf.keras.Model,
    model_ready_data: dict,
    model_config: ModelConfigSingular,
) -> pd.DataFrame:
    """Validate a regression model.

    Args:
    ----
        output (pd.DataFrame): The DataFrame containing the data to be validated.
        tf_model (tf.keras.Model): The trained TensorFlow regression model.
        model_ready_data (dict): The dictionary containing the model-ready data.

    Returns:
    -------
        pd.DataFrame: The DataFrame with an additional column for the model predictions.
    """
    output["model_prediction"] = tf_model.predict(
        model_ready_data["validation_split"]["validation_array"],
        verbose=2,
    ).reshape(-1)
    return output


def validate_model(
    model_name: str,
    config: ConfigFile,
    root_path: Path,
    dev_test_prod_setting: str = "test",
) -> None:
    """Validate a machine learning model.

    Args:
    ----
        model_name (str): The name of the model to validate.
        config (ConfigFile): The configuration file containing model
            and data settings.
        root_path (Path): The root path of the project.
        dev_test_prod_setting (str, optional): The setting for development,
            testing, or production. Defaults to "test".
    """
    build_model_ready_data(
        model_name=model_name,
        config=config,
        dev_test_prod_setting=dev_test_prod_setting,
        create_data_for="validation",
        root_path=root_path,
    )

    data = load_data_from_parquets(
        config,
        model_name,
        data_split="validation",
        root_path=root_path,
    )

    model_config = config.model_configurations[model_name]
    data_config = config.data_structures

    logging.info("Model %s Validation: Loading response variable..." % model_name)
    model_target_query = (
        f"SELECT {model_config.entity_id}, {model_config.response_variable_attribute_name} FROM "
        + get_schema(dev_test_prod_setting)
        + "."
        + model_config.source_response_variable_table_name
    )
    model_target = load_dataframe_from_string_query(model_target_query).set_index(
        model_config.entity_id,
    )

    model_target = model_target.merge(
        data["train_test_validation_split"][["train_test_validation_split"]],
        left_index=True,
        right_index=True,
    )
    del data["train_test_validation_split"]

    logging.info(
        "Model %s Validation: building model constructor config..." % model_name,
    )
    model_config = build_model_constructor_config(
        data,
        model_config,
        data_config,
    )

    logging.info("Model %s Validation: building model input..." % model_name)
    model_ready_data = build_model_input(
        data,
        model_config=model_config,
        left_most_ids=model_target,
    )

    logging.info("Model %s Validation: loading model..." % model_name)
    download_most_recent_tar_file(
        model_name=get_model_save_name(dev_test_prod_setting, model_name),
        object_name="tf_model",
        save_dir=root_path,
    )
    tf_model = tf.keras.models.load_model(str(root_path) + "/model_save")

    output = model_ready_data["validation_split"]["ordered_ids"]
    output = output.merge(
        model_ready_data["validation_split"]["target_dataframe"],
        on=model_config.entity_id,
    )

    if model_config.model_type == "binary_classifier":
        output = validate_binary_classifier(
            model_name=model_name,
            output=output,
            tf_model=tf_model,
            model_ready_data=model_ready_data,
            model_config=model_config,
        )

    elif model_config.model_type == "regression":
        output = validate_regression_model(
            output=output,
            tf_model=tf_model,
            model_ready_data=model_ready_data,
            model_config=model_config,
        )

    elif model_config.model_type == "multi_classifier":
        output = validate_multi_classifier(
            model_name=model_name,
            output=output,
            tf_model=tf_model,
            model_ready_data=model_ready_data,
            model_config=model_config,
        )

    output = output.astype(str)

    logging.info("Model %s Validation: Saving to DB..." % model_name)

    db_schema = get_schema(dev_test_prod_setting)

    query_drop = f"DROP TABLE IF EXISTS {db_schema}.model_{model_name}_validation"
    engine = create_engine()
    with Session(engine) as session:
        session.execute(query_drop)
        session.commit()

    source = DBConnector().source()
    source(
        schema=db_schema,
        table=f"model_{model_name}_validation",
    ).write_csv(output, index=False, overwrite=True)

    logging.info("Model %s Validation: Running Clean up..." % model_name)
    delete_dir = root_path / "model_save/"
    if delete_dir.is_dir():
        shutil.rmtree(str(delete_dir))
