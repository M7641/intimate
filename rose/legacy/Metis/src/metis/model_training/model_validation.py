import os

import numpy as np
import pandas as pd
import tensorflow as tf
from metis.configuration.configuration_classes import ExplainModel
from metis.utility_tools.connect import (drop_model_data_from_table,
                                         upload_data_to_table)
from metis.utility_tools.model_tools import return_response_variable_type
from sklearn import metrics


def model_validation_binary_classifier(
    model, data_dict: dict, model_config: ExplainModel
):
    """
    Performs model validation using the given model and data.

    Args:
        model (object): The trained model to be validated.
        data_dict (dict): A dictionary containing the data for validation.
        model_config (ExplainModel).

    Returns:
        None
    """
    raw_model_prediction = model.predict(
        np.asarray(data_dict["data_to_use_test"]).astype(np.float32),
        verbose=0,
    )

    validation = (
        data_dict["data_to_use_test"].index.to_frame().rename(columns={0: "table_id"})
    )
    validation["model_prediction"] = (raw_model_prediction > 0.5).astype(int)

    validation = validation.merge(
        pd.DataFrame(data_dict["target_test"]).reset_index(),
        how="left",
        on="table_id",
    )

    auc = metrics.roc_auc_score(validation["target"], validation["model_prediction"])
    f_one_score = metrics.f1_score(validation["target"], validation["model_prediction"])
    accuracy = metrics.accuracy_score(
        validation["target"], validation["model_prediction"]
    )

    print("Confusion Matrix:")
    print(
        tf.math.confusion_matrix(validation["target"], validation["model_prediction"])
    )
    print(
        f"Explainability model trained with: \n AUC: {auc:.3f} \n F1: {f_one_score:.3f}",
    )

    drop_model_data_from_table(
        os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"),
        "metis_explaining_model_validation",
        model_config.name,
    )

    data_to_save = pd.DataFrame(
        {
            "model_name": [model_config.name, model_config.name, model_config.name],
            "metric": ["AUC", "F1", "Accuracy"],
            "metric_value": [auc, f_one_score, accuracy],
        }
    )

    upload_data_to_table(
        os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"),
        "metis_explaining_model_validation",
        data_to_save,
    )


def model_validation_multi_classifier(
    model, data_dict: dict, model_config: ExplainModel
):
    raw_model_prediction = model.predict(
        np.asarray(data_dict["data_to_use_test"]).astype(np.float32),
        verbose=0,
    )

    validation = (
        data_dict["data_to_use_test"].index.to_frame().rename(columns={0: "table_id"})
    )
    validation["model_prediction"] = raw_model_prediction

    validation = validation.merge(
        pd.DataFrame(data_dict["target_test"]).reset_index(),
        how="left",
        on="table_id",
    )

    accuracy = metrics.accuracy_score(
        validation["target"], validation["model_prediction"]
    )

    print("Confusion Matrix:")
    print(
        tf.math.confusion_matrix(validation["target"], validation["model_prediction"])
    )
    print(f"Explainability model trained with: \n Accuracy: {accuracy:.3f}")

    drop_model_data_from_table(
        os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"),
        "metis_explaining_model_validation",
        model_config.name,
    )

    data_to_save = pd.DataFrame(
        {
            "model_name": [model_config.name],
            "metric": ["Accuracy"],
            "metric_value": [accuracy],
        }
    )

    upload_data_to_table(
        os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"),
        "metis_explaining_model_validation",
        data_to_save,
    )


def model_validation_regression(model, data_dict: dict, model_config: ExplainModel):
    raw_model_prediction = model.predict(
        np.asarray(data_dict["data_to_use_test"]).astype(np.float32),
        verbose=0,
    )

    validation = (
        data_dict["data_to_use_test"].index.to_frame().rename(columns={0: "table_id"})
    )
    validation["model_prediction"] = raw_model_prediction

    validation = validation.merge(
        pd.DataFrame(data_dict["target_test"]).reset_index(),
        how="left",
        on="table_id",
    )

    mse = metrics.mean_squared_error(
        validation["target"], validation["model_prediction"]
    )
    r2 = metrics.r2_score(validation["target"], validation["model_prediction"])

    print(f"Explainability model trained with: \n MSE: {mse:.3f} \n R2: {r2:.3f}")

    drop_model_data_from_table(
        os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"),
        "metis_explaining_model_validation",
        model_config.name,
    )

    data_to_save = pd.DataFrame(
        {
            "model_name": [model_config.name, model_config.name],
            "metric": ["MSE", "RSquared"],
            "metric_value": [mse, r2],
        }
    )

    upload_data_to_table(
        os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"),
        "metis_explaining_model_validation",
        data_to_save,
    )


def model_validation(model, data_dict: dict, model_config: ExplainModel):
    """
    Performs model validation using the given model and data.

    Args:
        model (object): The trained model to be validated.
        data_dict (dict): A dictionary containing the data for validation.
        model_config (ExplainModel).

    Returns:
        None
    """
    model_target_type = return_response_variable_type(data_dict["target_test"])
    if model_target_type in ["binary_classifier"]:
        model_validation_binary_classifier(model, data_dict, model_config)
    elif model_target_type in ["multi_classifier"]:
        model_validation_multi_classifier(model, data_dict, model_config)
    elif model_target_type in ["regression"]:
        model_validation_regression(model, data_dict, model_config)
    else:
        raise ValueError("Model target type not recognized.")
