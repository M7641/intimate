import logging
import shutil
from pathlib import Path

import numpy as np
import tensorflow as tf
from proteus.config.pydantic_models import ConfigFile
from proteus.connect import load_dataframe_from_string_query
from proteus.dev_test_prod_utils import get_model_save_name, get_schema
from proteus.model_tools.data_processing import (build_model_input,
                                                 load_data_from_parquets)
from proteus.model_tools.model_constructor import ModelConstructor
from proteus.model_tools.model_utils import (build_model_constructor_config,
                                             return_class_weights)
from proteus.workflow_steps.create_data_step import build_model_ready_data

# Everything breaks when this is imported earlier, no idea why.
import keras_tuner as kt  # isort: skip


def train_model(
    model_name: str,
    config: ConfigFile,
    root_path: Path,
    dev_test_prod_setting: str = "test",
) -> None:
    build_model_ready_data(
        model_name=model_name,
        config=config,
        dev_test_prod_setting=dev_test_prod_setting,
        create_data_for="train",
        root_path=root_path,
    )

    data = load_data_from_parquets(
        config,
        model_name=model_name,
        data_split="train",
        root_path=root_path,
    )
    model_config = config.model_configurations[model_name]
    data_config = config.data_structures

    logging.info("Model %s Training: Loading response variable..." % model_name)
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

    logging.info("Model %s Training: building model constructor config..." % model_name)
    model_config = build_model_constructor_config(
        data,
        model_config,
        data_config,
    )

    logging.info("Model %s Training: building model input..." % model_name)
    model_ready_data = build_model_input(
        data,
        model_config=model_config,
        left_most_ids=model_target,
    )

    model_constructor = ModelConstructor(
        model_config=model_config,
    )

    if model_config.model_type == "regression":
        class_weights = None
    else:
        class_weights = return_class_weights(
            model_ready_data["train_split"]["target_dataframe"],
            target_column=model_config.response_variable_attribute_name,
        )

        if len(class_weights) == 1:
            class_weights = None
            logging.warning(
                "Model %s Training: only one class in target variable, class weights not used"
                % model_name,
            )

    objective = [("val_loss", "min"), ("loss", "min")]
    tuner_file_name = root_path / "saved_model/"
    tuner = kt.BayesianOptimization(
        hypermodel=model_constructor.construct_model,
        objective=kt.Objective(*(objective[0])),
        max_trials=model_config.max_trials,
        num_initial_points=7,
        project_name=tuner_file_name,
    )

    training_data = model_ready_data["train_split"]["train_array"]
    logging.warning(training_data)
    training_target = np.asarray(
        model_ready_data["train_split"]["target_dataframe"][
            model_config.response_variable_attribute_name
        ],
    ).astype(np.float32)

    test_data = model_ready_data["test_split"]["test_array"]
    test_target = np.asarray(
        model_ready_data["test_split"]["target_dataframe"][
            model_config.response_variable_attribute_name
        ],
    ).astype(np.float32)

    def scheduler(epoch, lr):
        if epoch < 5:
            return lr
        else:
            return lr * tf.math.exp(-0.1)

    training_callbacks = [
        tf.keras.callbacks.EarlyStopping(
            monitor=objective[0][0],
            patience=model_config.early_stopping_patience,
        ),
        tf.keras.callbacks.LearningRateScheduler(scheduler),
    ]

    logging.info(
        "Model %s Training: Searching for best model (Keras tuner)..." % model_name,
    )
    tuner.search(
        training_data,
        training_target,
        batch_size=model_config.batch_size,
        epochs=model_config.epochs,
        validation_data=(test_data, test_target),
        class_weight=class_weights,
        callbacks=training_callbacks,
        verbose=model_config.training_verbose_level,
    )

    best_model = model_constructor.construct_model(
        tuner.get_best_hyperparameters()[0],
    )

    logging.info("Model %s Training: training best model from Keras tuner" % model_name)
    best_model.fit(
        training_data,
        training_target,
        validation_split=model_config.validation_split,
        batch_size=model_config.batch_size,
        epochs=model_config.epochs,
        verbose=model_config.training_verbose_level,
        class_weight=class_weights,
        callbacks=training_callbacks,
    )

    logging.info("Model %s Training: saving to s3" % model_name)
    model_constructor.save_model_to_s3(
        best_model=best_model,
        model_name=get_model_save_name(dev_test_prod_setting, model_name),
        save_dir=root_path,
    )

    if tuner_file_name.is_dir():
        shutil.rmtree(str(tuner_file_name))

    logging.info("Model %s Training: Model created!" % model_name)
