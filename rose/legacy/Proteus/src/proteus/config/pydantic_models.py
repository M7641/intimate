from typing import Literal

from pydantic import BaseModel, Field


class ManualEncodingChoices(BaseModel):
    one_hot_encoder: list | None = Field(
        None,
        nullable=True,
        description="The one hot encoder list.",
    )
    ordinal_encoder: list | None = Field(
        None,
        nullable=True,
        description="The ordinal encoder list.",
    )
    numerical_encoder: list | None = Field(
        None,
        nullable=True,
        description="The numerical encoder list.",
    )
    date_encoder: list | None = Field(
        None,
        nullable=True,
        description="The date encoder list.",
    )


class DataStructureSingular(BaseModel):
    entity_id: str = Field(
        "customer_id",
        description="The id of the entity to be explained.",
    )
    data_structure_name: str = Field(description="The name of the datastructure.")
    data_structure_type: Literal["single_row_relational", "event_as_array"] = Field(
        description="The type of the datastructure.",
    )
    source_train_table_name: str = Field(
        description="The name of the source table in the DB.",
    )
    source_predict_table_name: str = Field(
        description="The name of the prediction table in the DB.",
    )
    encoder_save_mode: Literal["s3"] = Field(
        "s3",
        description="The save mode of the encoder. Only S3 for now.",
    )
    date_minimum: str = Field(
        "2000-01-01",
        description="The minimum date for the data structure.",
    )
    event_as_array_sequence_length: int | None = Field(
        None,
        nullable=True,
        description="The sequence length of the event_as_array datastructure.",
    )
    event_as_array_date_column: str | None = Field(
        None,
        nullable=True,
        description="The date column of the event_as_array datastructure.",
    )
    ordinal_encoder_cut_off_point: int = Field(
        5,
        nullable=True,
        description="The cut off point for when an ordinal encoder is used rather than one hot.",
    )
    manual_encoding_choices: ManualEncodingChoices | None = Field(
        None,
        nullable=True,
        description="The manual encoding choices for the encoder.",
    )


class ModelConfigSingular(BaseModel):
    entity_id: str = Field(
        "customer_id",
        description="The id of the entity to be explained.",
    )
    model_name: str = Field(description="The name of the model.")
    model_type: Literal["binary_classifier", "multi_classifier", "regression"] = Field(
        description="The type of the model.",
    )
    source_response_variable_table_name: str = Field(
        description="The name of the response variable table in the DB.",
    )
    response_variable_attribute_name: str = Field(
        "target",
        description="The name of the response variable in the table.",
    )
    source_output_table_name: str = Field(
        description="The name of table of all Ids to generate predictions for.",
    )
    data_structures_used: list[str] = Field(
        description="The datastructures used in the model as defined in DataStructures.",
    )
    manual_train_test_split_table_name: str | None = Field(
        None,
        nullable=True,
        description="Table name for the manual train test split.",
    )
    number_of_classes: int = Field(1, description="The number of classes in the model.")
    epochs: int = Field(20, description="The number of epochs to train the model for.")
    batch_size: int = Field(128, description="The batch size to train the model with.")
    max_trials: int = Field(
        3,
        description="The maximum number of trials to run the model for.",
    )
    validation_split: float = Field(
        0.2,
        description="The validation split of the data to use for training.",
    )
    training_verbose_level: int = Field(
        0,
        description="The verbose level of the training.",
    )
    data_structure_config: dict = Field(
        None,
        nullable=True,
        description="The data structure config, to be filled in by code.",
    )
    early_stopping_patience: int = Field(
        5,
        description="The patience of the EarlyStopping callback.",
    )


class ConfigFile(BaseModel):
    model_configurations: dict[str, ModelConfigSingular]
    data_structures: dict[str, DataStructureSingular]
