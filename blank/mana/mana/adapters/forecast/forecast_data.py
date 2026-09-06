import os
from pathlib import Path

import polars as pl
from bhg.connect.DBObj import DBObj


def return_forecast_data(
    take_sample_products: int | None = None,
    validation_window: int = 0,
    print_query: bool = False,
    location_id: str | None = None,
) -> pl.DataFrame:
    schema = os.getenv("TARGET_SCHEMA", "PLT")
    db_manager = DBObj()

    db_schema = {
        "unique_id": pl.Utf8,
        "ds": pl.Date,
        "category_level1": pl.Categorical,
        "category_level2": pl.Categorical,
        "category_level3": pl.Categorical,
        "category_level4": pl.Categorical,
        "price_ranking": pl.Float32,
        "elasticity": pl.Float32,
        "y": pl.Float32,
    }

    return db_manager.query(
        query=Path(__file__).parent.resolve() / "sql/forecast_data.sql.j2",
        params={
            "schema": schema if schema else "PLT",
            "take_sample_products": take_sample_products,
            "pricing_elasticity_results": "pricing_elasticity_results",
            "location_id": location_id,
            "validation_window": validation_window,
        },
        schema=db_schema,
        return_polars=True,
        print_query=print_query,
    )


def return_dynamic_data(
    print_query: bool = False,
    location_id: str | None = None,
    unique_ids: list[str] | None = None,
    config: dict[str, int] | None = None,
) -> pl.DataFrame:
    if config is None:
        config = {"horizon": 0, "history": 0, "validation_window": 0}
    db_manager = DBObj()
    schema = os.getenv("TARGET_SCHEMA", "PLT")

    return db_manager.query(
        query=Path(__file__).parent.resolve() / "sql/dynamic_features_data.sql.j2",
        params={
            "schema": schema if schema else "PLT",
            "horizon": config["horizon"],
            "history": config["history"],
            "validation_window": config["validation_window"],
            "location_id": location_id,
            "unique_ids": unique_ids,
        },
        return_polars=True,
        print_query=print_query,
    )
