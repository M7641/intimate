import pandas as pd
from proteus.config.pydantic_models import (DataStructureSingular,
                                            ModelConfigSingular)


def return_class_weights(
    target_data: pd.DataFrame,
    target_column: str,
) -> dict:
    weights_pre = (
        target_data.groupby(target_column)
        .size()
        .rename("positives")
        .reset_index()
        .assign(
            total=len(target_data),
            classes=target_data[target_column].nunique(),
        )
    )

    weights = (
        1 / weights_pre["positives"] * weights_pre["total"] / weights_pre["classes"]
    )
    class_weight = dict(zip(weights_pre[target_column], weights))

    return class_weight


def build_model_constructor_config(
    data: dict,
    model_config: ModelConfigSingular,
    data_config: dict[str, DataStructureSingular],
) -> ModelConfigSingular:
    input_order = 1
    new_data_config = {}
    for key, value in data.items():
        if data_config[key].data_structure_type == "event_as_array":
            total_features = value.values[0, 0].shape[0]
        else:
            total_features = None

        new_data_config[key] = {
            "data_shape": value.shape,
            "input_order": input_order,
            "data_structure_type": data_config[key].data_structure_type,
            "total_features": total_features,
        }
        input_order += 1

    model_config.data_structure_config = new_data_config

    return model_config
