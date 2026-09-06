from pathlib import Path

import numpy as np
import pandas as pd
from proteus.config.pydantic_models import ModelConfigSingular


def load_data_from_parquets(
    config: ModelConfigSingular,
    model_name: str,
    root_path: Path,
    data_split: str = "train",
) -> dict:
    """Load data from parquet files.

    Parameters
    ----------
        model_config (dict): The configuration of the model.
        model_name (str): The name of the model.
        data_split (str, optional): The data split to load. Defaults to "train".
        root_path (Path | None, optional): The root path where the parquet files are located. Defaults to None.

    Returns
    -------
        dict: A dictionary containing the loaded data.
    """
    data_structures_to_load = config.model_configurations[
        model_name
    ].data_structures_used
    data = {
        i: pd.read_parquet(
            f"{root_path}/data/{j.data_structure_name}_{data_split}.parquet.gzip",
        )
        for i, j in config.data_structures.items()
        if i in data_structures_to_load
    }

    if data_split in ["train", "validation"]:
        train_test_split = pd.read_parquet(
            root_path / "data/train_test_validation_split.parquet.gzip",
        )
        data["train_test_validation_split"] = train_test_split

    return data


def wrap_data_into_arrays(
    data: dict,
    model_config: ModelConfigSingular,
    left_most_ids: pd.DataFrame,
) -> pd.DataFrame:
    """Wraps the data into arrays based on the specified data structure type in the model configuration.
    Neural network models require the data to be in arrays.

    Args:
    ----
        data (dict): A dictionary containing the data frames to be wrapped.
        model_config (dict): A dictionary containing the model configuration.
        left_most_ids (pd.DataFrame): The left-most IDs data frame.

    Returns:
    -------
        pd.DataFrame: The wrapped data frame with arrays.

    """
    for key, data_frame in data.items():
        data_structure_type = model_config.data_structure_config[key][
            "data_structure_type"
        ]

        if data_structure_type == "single_row_relational":
            data_frame_join = data_frame.astype(np.float32).apply(
                lambda x: x.to_numpy(),
                axis=1,
            )
            data_frame_join = pd.DataFrame(data_frame_join, columns=[key])

        elif data_structure_type == "event_as_array":
            data_frame_join = data_frame.apply(lambda x: np.array(x), axis=1)
            data_frame_join = data_frame_join.map(
                lambda x: np.stack([np.array(i) for i in x], axis=0).astype(np.float32),
            )
            data_frame_join = pd.DataFrame(data_frame_join, columns=[key])

        left_most_ids = left_most_ids.merge(
            data_frame_join,
            left_index=True,
            right_index=True,
            how="left",
        )
        null_rows = left_most_ids[key].isna().sum()

        if null_rows > 0:
            take_a_normal_row = left_most_ids[key][~left_most_ids[key].isna()].iloc[0]
            fill_data = pd.DataFrame([np.zeros(1)] * null_rows).apply(
                lambda x: x.to_numpy(),
                axis=1,
            )
            left_most_ids.loc[left_most_ids.isna()[key], key] = fill_data.to_numpy()
            left_most_ids[key] = left_most_ids[key].apply(
                lambda x: np.resize(x, take_a_normal_row.shape),
            )

    return left_most_ids.sort_index(ascending=True)


def build_model_input(
    data: dict,
    model_config: ModelConfigSingular,
    left_most_ids: pd.DataFrame,
) -> dict:
    left_most_ids = wrap_data_into_arrays(data, model_config, left_most_ids)

    model_ready_data_dict: dict = {}
    for i in ["train", "test", "validation"]:
        out_put_data = left_most_ids.query(f"train_test_validation_split == '{i}'")
        data_as_array: dict = {}
        for key in data:
            data_as_array[f"inputs_{key}"] = np.stack(out_put_data[key].values, axis=0)

        model_ready_data_dict[f"{i}_split"] = {
            "ordered_ids": out_put_data.reset_index()[[model_config.entity_id]],
            f"{i}_array": data_as_array,
            "target_dataframe": out_put_data.reset_index()[
                [model_config.entity_id, model_config.response_variable_attribute_name]
            ],
        }

    return model_ready_data_dict
