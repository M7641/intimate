import polars as pl

from mana.encoders.encoder_protocol import EncoderProtocol
from mana.encoders.label_encoder import LabelEncoder
from mana.encoders.numeric_encoder import NumericEncoder


def create_encoders(data_schematics: list[dict]) -> dict[str, EncoderProtocol]:
    """
    Create encoders based on the provided data schematics.
    Each encoder is initialized but not yet fitted.

    Args:
        data_schematics (list[dict]): A list of data schematic dictionaries,
            each containing at least a "name" and "type" key.
    """
    store_encoders = {}
    for data_schematic in data_schematics:
        if data_schematic["type"] in ["categorical", "identifier"]:
            encoder = LabelEncoder(
                name=str(data_schematic.get("name")) + "_encoder",
                column_key=str(data_schematic.get("name")),
            )
        elif data_schematic["type"] == "numeric":
            encoder = NumericEncoder(
                name=str(data_schematic.get("name")) + "_encoder",
                column_key=str(data_schematic.get("name")),
            )
        else:
            msg = f"Unknown feature type: {data_schematic['type']}"
            raise ValueError(msg)

        store_encoders[str(data_schematic.get("name")) + "_encoder"] = encoder
    return store_encoders


def fit_encoders(
    data_schematics: list[dict],
    df: pl.DataFrame,
    stored_encoders: dict[str, EncoderProtocol],
) -> dict[str, EncoderProtocol]:
    """
    Fit encoders based on the provided data schematics and DataFrame.

    Args:
        data_schematics (list[dict]): A list of data schematic dictionaries,
            each containing at least a "name" and "type" key.
        df (pl.DataFrame): The Polars DataFrame containing the data to fit the encoders.
        stored_encoders (dict[str, EncoderProtocol]): A dictionary of initialized encoders.
    """
    for data_schematic in data_schematics:
        encoder = stored_encoders.get(
            str(data_schematic.get("name")) + "_encoder",
        )
        if encoder is None:
            msg = f"No encoder found for feature: {data_schematic.get('name')}"
            raise ValueError(msg)

        if data_schematic["type"] in ["categorical", "identifier"] and isinstance(
            encoder, LabelEncoder
        ):
            encoder.fit(df.select(data_schematic["name"]).to_series().to_list())
        elif data_schematic["type"] == "numeric" and isinstance(
            encoder, NumericEncoder
        ):
            encoder.fit(df.select(data_schematic["name"]).to_series().to_list())
        else:
            msg = f"Unknown feature type: {data_schematic['type']}"
            raise ValueError(msg)

    return stored_encoders


def apply_encoders(
    data_schematics: list[dict],
    df: pl.DataFrame,
    stored_encoders: dict[str, EncoderProtocol],
) -> pl.DataFrame:
    """
    Apply fitted encoders to the provided DataFrame.

    Args:
        data_schematics (list[dict]): A list of data schematic dictionaries,
            each containing at least a "name" and "type" key.
        df (pl.DataFrame): The Polars DataFrame containing the data to transform.
        stored_encoders (dict[str, EncoderProtocol]): A dictionary of fitted encoders.
    """

    for data_schematic in data_schematics:
        encoder = stored_encoders.get(
            str(data_schematic.get("name")) + "_encoder",
        )
        if encoder is None:
            msg = f"No encoder found for feature: {data_schematic.get('name')}"
            raise ValueError(msg)

        if encoder.has_been_fitted is False:
            msg = (
                f"Encoder for feature {data_schematic.get('name')} has not been fitted."
            )
            raise ValueError(msg)

        if data_schematic["type"] in ["categorical", "identifier"] and isinstance(
            encoder, LabelEncoder
        ):
            df = df.with_columns(
                [
                    (
                        pl.col(data_schematic["name"])
                        .replace_strict(encoder.label_to_index, default=-1)
                        .cast(pl.Int64)
                        .alias(data_schematic["name"])
                    ),
                ]
            )

        elif data_schematic["type"] == "numeric" and isinstance(
            encoder, NumericEncoder
        ):
            df = df.with_columns(
                [
                    pl.col(data_schematic["name"])
                    .map_elements(encoder.transform, return_dtype=pl.Float64)
                    .alias(data_schematic["name"]),
                ]
            )

        else:
            msg = f"Unknown feature type: {data_schematic['type']}"
            raise ValueError(msg)

    return df
