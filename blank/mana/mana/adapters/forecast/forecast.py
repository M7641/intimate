import os

import polars as pl

from bhg.connect.DBObj import DBObj
from mana.adapters.forecast.forecast_data import (
    return_dynamic_data,
    return_forecast_data,
)
from mana.adapters.forecast.models.lstm import LSTM
from mana.adapters.forecast.models.metrics import mean_absolute_error
from mana.adapters.forecast.window_generator import WindowGenerator


def get_static_features() -> list[str]:
    return [
        "category_level1",
        "category_level2",
        "category_level3",
        "category_level4",
        "price_ranking",
        "elasticity",
    ]


def get_dynamic_features() -> list[str]:
    return ["seasonality_index"]


def save_to_db_wrap(
    output: pl.DataFrame,
    overwrite_mode: str = "truncate",
    table_name: str = "test_ml_forecast",
) -> None:
    db_manager = DBObj()
    schema = os.getenv("TARGET_SCHEMA", "PLT")

    overwrite = bool(overwrite_mode == "truncate")

    if overwrite:
        query = f"""
        TRUNCATE TABLE IF EXISTS {schema}.{table_name}
        """
        db_manager.query(query)

    db_manager.write_to_db(
        schema=schema,
        table=table_name,
        df=output,
        overwrite=overwrite,
    )


def prepare_output_for_db(
    output: pl.DataFrame,
    window_maker: WindowGenerator,
    predict_horizon: int,
) -> pl.DataFrame:
    return (
        output.with_columns(
            pl.col(window_maker.config.id_column)
            .str.split("-")
            .list.to_struct()
            .struct.rename_fields(["product_id", "location_id"])
            .alias("unnest_me"),
            pl.lit(predict_horizon).alias("horizon"),
            pl.lit("ml_forecast").alias("method"),
            pl.lit(
                str(output.get_column(window_maker.config.date_column).max()),
            ).alias("data_cutoff_date"),
            pl.lit(None).alias("actual"),
        )
        .unnest("unnest_me")
        .rename({window_maker.config.date_column: "data_start_date"})
        .select(
            [
                "product_id",
                "location_id",
                "data_cutoff_date",
                "data_start_date",
                "predicted",
                "actual",
                "horizon",
                "method",
            ],
        )
        .with_columns(pl.col("predicted").round(4))
    )


def run_forecast(
    validation_window: int = 4,
) -> None:

    predict_horizon = 12
    history = 40

    sample_products = None
    forecast_data = return_forecast_data(
        take_sample_products=sample_products,
        location_id="UK",
        validation_window=validation_window,
        print_query=False,
    )

    if sample_products:
        unique_ids = [i[0] for i in forecast_data.select("unique_id").to_numpy()]
    else:
        unique_ids = None

    dynamic_data = return_dynamic_data(
        location_id="UK",
        unique_ids=unique_ids,
        config={
            "horizon": predict_horizon,
            "history": history,
            "validation_window": validation_window,
        },
    )

    forecast_data = forecast_data.join(
        dynamic_data,
        on=["unique_id", "ds"],
        how="left",
    )

    static_features = get_static_features()
    dynamic_features = get_dynamic_features()

    window_maker = WindowGenerator(
        data=forecast_data,
        config={
            "input_width_weeks": history - predict_horizon,
            "target_width_weeks": predict_horizon,
            "static_feature_columns": static_features,
            "dynamic_feature_columns": dynamic_features,
        },
    )

    lstm = LSTM(
        lstm_config={
            "static_features_columns": static_features,
            "dynamic_feature_columns": dynamic_features,
            "column_indices": window_maker.column_indices,
            "columns_cardinality": window_maker.categories_to_embed,
        },
        horizon=predict_horizon,
    )

    lstm.compile(
        metrics=[mean_absolute_error],
        optimizer=lstm.return_optimiser(),
    )

    lstm.fit(
        {
            "train": window_maker.train,
            "future_dynamic_features": window_maker.test_dynamic_features,
            "static_features": window_maker.static_feature_slice,
        },
        window_maker.test_target,
        verbose=1,
        epochs=100,
        validation_split=0.1,
        callbacks=lstm.collect_some_lovely_callbacks(),
        batch_size=32,
    )

    dynamic_features = window_maker.generate_future_date_spine(predict_horizon).join(
        dynamic_data,
        on=["unique_id", "ds"],
        how="left",
    )

    dynamic_features = window_maker.process_dynamic_features(
        future_dynamic_features=dynamic_features,
        horizon=lstm.horizon,
    )

    final_predictions = lstm.predict_step(
        future_dynamic_features=dynamic_features,
        final_target_value=window_maker.last_target_value,
        static_features_single=window_maker.static_feature_slice,
    )

    output = window_maker.build_final_output(predictions=final_predictions)

    output = prepare_output_for_db(
        output=output,
        window_maker=window_maker,
        predict_horizon=predict_horizon,
    )

    save_to_db_wrap(
        output=output,
        overwrite_mode="truncate",
        table_name="validation_forecast" if validation_window else "test_ml_forecast",
    )
