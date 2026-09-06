from pathlib import Path
from typing import Literal

import pandas as pd
from proteus.config.pydantic_models import ConfigFile
from proteus.connect import load_dataframe_from_string_query
from proteus.dev_test_prod_utils import get_schema
from proteus.io_tools import save_to_parquet
from proteus.model_tools.data_constructor import DataConstructor


def build_model_ready_data(
    model_name: str,
    config: ConfigFile,
    root_path: Path,
    create_data_for: Literal["train", "validation", "predict"],
    dev_test_prod_setting: str = "test",
) -> None:
    model_config = config.model_configurations[model_name]
    data_config = {
        i: j
        for i, j in config.data_structures.items()
        if i in model_config.data_structures_used
    }

    for data_entity_config in data_config.values():
        data_constructor = DataConstructor(
            data_config=data_entity_config,
            save_append_name=model_name,
        )

        if create_data_for in ["train", "validation"]:
            source_table = data_entity_config.source_train_table_name
        elif create_data_for == "predict":
            source_table = data_entity_config.source_predict_table_name
        else:
            raise ValueError(
                "create_data_for must be 'train', 'validation', or 'predict'.",
            )

        build_sql_query = (
            "SELECT * FROM " + get_schema(dev_test_prod_setting) + "." + source_table
        )
        data = load_dataframe_from_string_query(build_sql_query).set_index(
            model_config.entity_id,
        )

        if create_data_for == "train":
            train_test_split = pd.read_parquet(
                root_path / "data" / "train_test_validation_split.parquet.gzip",
            )
            data = data.merge(
                train_test_split[["train_test_validation_split"]],
                on=model_config.entity_id,
            )

            train_data_to_process = data[
                data["train_test_validation_split"] == "train"
            ].drop(
                columns=["train_test_validation_split"],
            )
            train_data = data_constructor.process_data_for_model(
                data=train_data_to_process,
                train_encoders="train",
            )

            test_data_to_process = data[
                data["train_test_validation_split"] == "test"
            ].drop(
                columns=["train_test_validation_split"],
            )
            test_data = data_constructor.process_data_for_model(
                data=test_data_to_process,
                train_encoders="use_saved",
            )

            processed_data = pd.concat([train_data, test_data])

        elif create_data_for == "validation":
            train_test_split = pd.read_parquet(
                root_path / "data" / "train_test_validation_split.parquet.gzip",
            )
            data = data.merge(
                train_test_split[["train_test_validation_split"]],
                on=model_config.entity_id,
            )
            data = data[data["train_test_validation_split"] == "validation"].drop(
                columns=["train_test_validation_split"],
            )
            processed_data = data_constructor.process_data_for_model(
                data=data,
                train_encoders="use_saved",
            )

        elif create_data_for == "predict":
            processed_data = data_constructor.process_data_for_model(
                data=data,
                train_encoders="use_saved",
            )

        else:
            raise ValueError(
                "create_data_for must be 'train', 'validation', or 'predict'",
            )

        save_to_parquet(
            data=processed_data,
            dir_name=root_path,
            data_name=data_entity_config.data_structure_name + "_" + create_data_for,
        )
