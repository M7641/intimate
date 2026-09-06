import logging
import shutil
from pathlib import Path

import numpy as np
from proteus.config.pydantic_models import ConfigFile
from proteus.connect import (DBConnector, create_engine,
                             load_dataframe_from_string_query)
from proteus.dev_test_prod_utils import get_model_save_name, get_schema
from proteus.model_tools.data_processing import (load_data_from_parquets,
                                                 wrap_data_into_arrays)
from proteus.model_tools.model_utils import build_model_constructor_config
from proteus.model_tools.s3_tools import download_most_recent_tar_file
from proteus.workflow_steps.create_data_step import build_model_ready_data
from sqlalchemy.orm import Session

# Everything breaks when this is imported earlier, no idea why.
import tensorflow as tf  # isort: skip


def create_final_model_outputs(
    model_name: str,
    config: ConfigFile,
    root_path: Path,
    dev_test_prod_setting: str = "test",
) -> None:
    build_model_ready_data(
        model_name=model_name,
        config=config,
        root_path=root_path,
        create_data_for="predict",
        dev_test_prod_setting=dev_test_prod_setting,
    )

    data = load_data_from_parquets(
        config,
        model_name,
        data_split="predict",
        root_path=root_path,
    )

    model_config = config.model_configurations[model_name]
    data_config = config.data_structures

    model_config = build_model_constructor_config(
        data,
        model_config,
        data_config,
    )

    output_ids_query = (
        f"SELECT {model_config.entity_id}, take_for_outputs FROM "
        + get_schema(dev_test_prod_setting)
        + "."
        + model_config.source_output_table_name
        + " WHERE take_for_outputs = 1"
    )
    output_ids = load_dataframe_from_string_query(output_ids_query).set_index(
        model_config.entity_id,
    )

    model_ready_data = wrap_data_into_arrays(
        data,
        model_config=model_config,
        left_most_ids=output_ids,
    )

    stacked_output_data: dict = {}
    for k in model_config.data_structures_used:
        stacked_output_data[f"inputs_{k}"] = np.stack(
            model_ready_data[k].values,
            axis=0,
        )

    model_file_location = root_path / "previous_model/"
    model_file_location.mkdir(parents=True, exist_ok=True)
    download_most_recent_tar_file(
        model_name=get_model_save_name(dev_test_prod_setting, model_name),
        object_name="tf_model",
        save_dir=model_file_location,
    )
    tf_model = tf.keras.models.load_model(str(root_path) + "/previous_model/model_save")

    msg = f"\n Predict for Model: {model_name} \n"
    logging.info(msg)

    output = model_ready_data.reset_index()[[model_config.entity_id]].copy(deep=True)

    if model_config.model_type == "binary_classifier":
        output["model_prediction"] = tf_model.predict(
            stacked_output_data,
            verbose=2,
        ).reshape(-1)

    elif model_config.model_type == "regression":
        output["model_prediction"] = tf_model.predict(
            stacked_output_data,
            verbose=2,
        ).reshape(-1)

    elif model_config.model_type == "multi_classifier":
        output["model_prediction"] = np.argmax(
            tf_model.predict(
                stacked_output_data,
                verbose=2,
            ),
            axis=1,
        )

    db_schema = get_schema(dev_test_prod_setting)

    query_drop = f"DROP TABLE IF EXISTS {db_schema}.model_{model_name}_outputs"
    engine = create_engine()
    with Session(engine) as session:
        session.execute(query_drop)
        session.commit()

    source = DBConnector().source()
    source(
        schema=db_schema,
        table=f"model_{model_name}_outputs",
    ).write_csv(output, index=False, overwrite=True)

    if model_file_location.is_dir():
        shutil.rmtree(str(model_file_location))

    msg = f"\n Completed Predict for Model: {model_name} \n"
    logging.info(msg)
