from pathlib import Path

import numpy as np
import pandas as pd
from proteus.config.pydantic_models import ConfigFile
from proteus.connect import load_dataframe_from_string_query
from proteus.dev_test_prod_utils import get_schema
from proteus.io_tools import save_to_parquet
from sklearn.model_selection import train_test_split


def build_train_test_validation_split(
    model_name: str,
    config: ConfigFile,
    dev_test_prod_setting: str,
    root_path: Path,
) -> pd.DataFrame:
    """Generate a train/test/validation split for a given dataset.

    Args:
    ----
        config (dict): A dictionary containing configuration settings.
        dev_test_prod_setting (str): The setting indicating the environment
            (development, testing, production).
        path (Path): The path to the directory.

    Returns:
    -------
        pd.DataFrame: The dataset with an additional column indicating the split
            (train, test, or validation).
    """
    model_config = config.model_configurations[model_name]

    if model_config.manual_train_test_split_table_name:
        query_str = (
            "SELECT * FROM "
            + get_schema(dev_test_prod_setting)
            + "."
            + model_config.manual_train_test_split_table_name
        )
        data_frame = load_dataframe_from_string_query(query_str).set_index(
            model_config.entity_id,
        )

        if "train_test_validation_split" not in data_frame.columns:
            msg = (
                "train_test_validation_split column not",
                " found in manual_train_test_split_table_name",
            )
            raise ValueError(msg)
    else:
        source_table = model_config.source_response_variable_table_name
        query_str = (
            "SELECT * FROM " + get_schema(dev_test_prod_setting) + "." + source_table
        )
        data_frame = load_dataframe_from_string_query(query_str).set_index(
            model_config.entity_id,
        )

        train, pre_test = train_test_split(
            data_frame,
            test_size=0.3,
            random_state=42,
            stratify=data_frame[model_config.response_variable_attribute_name]
            if model_config.model_type == "binary_classifier"
            else None,
        )

        test, validation = train_test_split(
            pre_test,
            test_size=0.50,
            random_state=42,
            stratify=pre_test[model_config.response_variable_attribute_name]
            if model_config.model_type == "binary_classifier"
            else None,
        )

        conditions = [
            data_frame.index.isin(train.index),
            data_frame.index.isin(test.index),
            data_frame.index.isin(validation.index),
        ]
        choices = ["train", "test", "validation"]
        data_frame["train_test_validation_split"] = np.select(
            conditions,
            choices,
            default="validation",
        )

    save_to_parquet(
        data=data_frame,
        dir_name=root_path,
        data_name="train_test_validation_split",
    )
