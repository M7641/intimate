import os

import numpy as np
import pandas as pd
from metis.configuration.configuration_classes import ExplainModel
from metis.model_training.feed_forward_model_constructor import \
    FeedForwardConstructor
from metis.model_training.lightgbm_model_constructor import LightGBMConstructor
from metis.model_training.model_validation import model_validation
from metis.utility_tools.connect import (drop_model_data_from_table,
                                         upload_data_to_table)
from metis.utility_tools.data_constructor import DataConstructor
from metis.utility_tools.model_tools import return_response_variable_type
from metis.utility_tools.s3_tools import upload_to_s3
from sklearn.model_selection import train_test_split


def upload_model_predictions(model, data: pd.DataFrame, model_config: ExplainModel):
    """
    Uploads the predictions of the model to S3.

    Args:
        model: The model.
        data: The data.
        model_name: The name of the model.
    """

    drop_model_data_from_table(
        os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"),
        "metis_model_explainability_output_predictions",
        model_config.name,
    )

    if model_config.model_type_to_use == "neural_network":
        data_as_array = np.asarray(data).astype(np.float32)
        predictions = model.predict(data_as_array)
    elif model_config.model_type_to_use == "lightgbm" and model_config.model_type in [
        "binary_classifier",
        "multiclass_classifier",
    ]:
        predictions = model.predict_proba(data)[:, 1]
    else:
        predictions = model.predict(data)

    data_to_save = data.index.to_frame().rename(columns={0: "table_id"})
    data_to_save["model_prediction"] = predictions
    data_to_save["model_name"] = model_config.name

    if model_config.model_label_setting.use_bins:
        data_to_save["model_prediction_binned"] = pd.cut(
            data_to_save["model_prediction"],
            bins=len(model_config.model_label_setting.bin_labels),
            labels=model_config.model_label_setting.bin_labels,
        )
    elif model_config.model_label_setting.use_quartiles:
        data_to_save["model_prediction_binned"] = pd.qcut(
            data_to_save["model_prediction"],
            q=[
                1 / i
                for i in range(1, len(model_config.model_label_setting.bin_labels))
            ],
            labels=model_config.model_label_setting.bin_labels,
        )
    else:
        data_to_save["model_prediction_binned"] = data_to_save["model_prediction"]

    upload_data_to_table(
        os.getenv("OUTPUT_DB_SCHEMA", "PUBLISH"),
        "metis_model_explainability_output_predictions",
        data_to_save,
    )


def train_lightgbm_model(data: dict, target: pd.DataFrame):
    """
    Trains a LightGBM model using the given data.

    Returns:
        model: The trained LightGBM model.
    """
    model = LightGBMConstructor(parameters=None).fit(data, target)
    return model


def train_neural_network(data: dict, target: pd.DataFrame):
    """
    Trains a neural network model using the given data.

    Returns:
        model: The trained neural network model.
    """
    model = FeedForwardConstructor().fit(data, target)
    return model


def train_meta_model(
    training_data: pd.DataFrame,
    target: pd.DataFrame,
    model_config: ExplainModel,
):
    """
    Trains a meta model using the given training data and target.

    Args:
        training_data (pd.DataFrame): The training data.
        target (pd.DataFrame): The target data.
        entity_id (str): The ID of the table.
        show_validation (bool, optional): Whether to show validation. Defaults to False.

    Returns:
        model: The trained meta model.
    """

    training_data = training_data.sort_values("table_id")
    target = target.sort_values("table_id").set_index("table_id")
    data_constructor = DataConstructor(save_append_name=model_config.name)

    processed_data = data_constructor.process_data_for_model(
        training_data.set_index("table_id"),
        train_encoders="train",
    )

    model_target_type = return_response_variable_type(target)

    train_test_split_lists = train_test_split(
        processed_data,
        target,
        test_size=0.15,
        stratify=target if model_target_type in ["binary_classifier"] else None,
    )

    train_test_split_arrays_names = [
        "data_to_use_train",
        "data_to_use_test",
        "target_train",
        "target_test",
    ]

    data_dict = dict(zip(train_test_split_arrays_names, train_test_split_lists))

    if model_config.model_type_to_use == "neural_network":
        model = train_neural_network(data_dict, target)
    elif model_config.model_type_to_use == "lightgbm":
        model = train_lightgbm_model(data_dict, target)

    model_validation(model, data_dict, model_config)

    upload_model_predictions(model, processed_data, model_config)

    upload_to_s3(
        file=model,
        model_name=model_config.name,
        object_name="explainability_models",
    )
