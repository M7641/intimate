import datetime
import itertools

import numpy as np
import polars as pl
import tensorflow as tf
from pydantic import BaseModel, Field


class WindowGeneratorConfig(BaseModel):
    input_width_weeks: int = Field(..., gt=0)
    target_width_weeks: int = Field(..., gt=0)
    static_feature_columns: list[str] = Field(default_factory=list)
    dynamic_feature_columns: list[str] = Field(default_factory=list)
    validation_width_weeks: int = Field(default=0, ge=0)
    target_column: str = Field(default="y")
    id_column: str = Field(default="unique_id")
    date_column: str = Field(default="ds")
    date_unit: str = Field(default="week")


class WindowGenerator:
    def __init__(
        self,
        data: pl.DataFrame,
        config: dict,
    ) -> None:

        self.config = WindowGeneratorConfig(**config)

        self.numeric_encodings: dict[str, dict[str, float]] = {}
        self.categorical_encodings = {}

        self.static_feature_values: pl.DataFrame | None = None

        self.total_window_size: int = (
            self.config.input_width_weeks
            + self.config.target_width_weeks
            + self.config.validation_width_weeks
        )

        self.most_recent_date = data.select(pl.max(self.config.date_column))[0, 0]
        self.data = self.convert_pl_frame_to_tensor(data)

    def encode_data(self, data: pl.DataFrame) -> pl.DataFrame:
        data = self.encode_id_column(data)
        data = self.encode_categorical_features(data)
        data = self.encode_numeric_features(data)
        self.static_feature_values = self.create_static_feature_columns(data)
        return data

    def encode_id_column(self, data: pl.DataFrame) -> pl.DataFrame:
        self.categorical_encodings[self.config.id_column] = {
            "mapping": self.create_id_mapping(data, self.config.id_column),
            "cardinality": -1,
        }
        return self.map_id_to_unique_id(data, self.config.id_column)

    def encode_numeric_features(
        self,
        data: pl.DataFrame,
        train_mode: bool = True,
    ) -> pl.DataFrame:
        # normalise the numeric features
        numeric_columns = data.select(
            pl.selectors.numeric() - pl.selectors.contains(self.config.id_column),
        ).columns

        for column in numeric_columns:
            if train_mode:
                self.numeric_encodings[column] = {
                    "mean": data.select(column).mean()[0, 0],
                    "std": data.select(column).std()[0, 0],
                }
            data = data.with_columns(
                pl.col(column)
                - (
                    self.numeric_encodings[column]["mean"]
                    / self.numeric_encodings[column]["std"]
                ),
            )

        return data

    def encode_categorical_features(self, data: pl.DataFrame) -> pl.DataFrame:
        categorical_columns = data.select(
            (pl.selectors.categorical() | pl.selectors.string())
            - pl.selectors.by_name(self.config.date_column, self.config.id_column),
        ).columns

        for column in categorical_columns:
            self.categorical_encodings[column] = {
                "mapping": self.create_id_mapping(data, column),
                "cardinality": data.select(column).n_unique(),
            }
            data = self.map_id_to_unique_id(data, column)

        return data

    def map_id_to_unique_id(self, data: pl.DataFrame, column_name: str) -> pl.DataFrame:
        return data.with_columns(
            pl.col(column_name)
            .map_elements(
                lambda x: self.categorical_encodings[column_name]["mapping"][x],
                return_dtype=pl.Int64,
            )
            .cast(pl.Int64)
            .alias(column_name),
        )

    def create_id_mapping(self, data: pl.DataFrame, column_name: str) -> dict[str, int]:
        id_mapping = (
            data.select(column_name)
            .unique()
            .sort(by=column_name)
            .with_columns(pl.col(column_name).rank().cast(pl.Int64).alias("id"))
            .to_dicts()
        )
        return {i[column_name]: i["id"] for i in id_mapping}

    def create_static_feature_columns(self, data: pl.DataFrame) -> pl.DataFrame:
        static_feature_df = data.group_by(self.config.id_column).agg(
            **{i: pl.col(i).first() for i in self.config.static_feature_columns}
        )

        return static_feature_df.with_columns(
            *[pl.col(i).fill_null(0) for i in self.config.static_feature_columns],
        )

    def allocate_indices(self, data: pl.DataFrame) -> None:
        self.column_indices = {name: i for i, name in enumerate(data.columns)}
        self.target_column_index = self.column_indices[self.config.target_column]
        self.id_column_index = self.column_indices[self.config.id_column]
        self.static_feature_columns_indices = [
            self.column_indices[i] for i in self.config.static_feature_columns
        ]
        self.dynamic_feature_columns_indices = [
            self.column_indices[i] for i in self.config.dynamic_feature_columns
        ]

    def convert_pl_frame_to_tensor(
        self,
        data: pl.DataFrame,
    ) -> tf.Tensor:
        data = self.encode_data(data)
        data = data.sort(by=self.config.date_column, descending=False)

        data = data.select(
            [
                self.config.id_column,
                *self.config.static_feature_columns,
                *self.config.dynamic_feature_columns,
                self.config.target_column,
            ]
        )

        self.allocate_indices(data)

        # sort the columns into the right order for an array.
        columns = list(self.column_indices.keys())
        columns = sorted(columns, key=lambda x: self.column_indices[x])
        data = data.select(columns)

        return self.process_checked_data(
            data,
            sequence_length=self.total_window_size,
        )

    def process_checked_data(
        self,
        data: pl.DataFrame,
        sequence_length: int = 10,
        dyanmic_only: bool = False,
    ) -> pl.DataFrame:
        """If dyanmic_only is True, then only the dynamic features will be processed.

        This means we will not repair static freatures nor attempt to fill them.
        Static features should never be null so that will error if they are.
        """
        if dyanmic_only:
            repair_index = [self.id_column_index]
        else:
            repair_index = [self.id_column_index, *self.static_feature_columns_indices]
            data = data.with_columns(
                [pl.col(i).fill_null(0) for i in self.config.static_feature_columns],
            )

        data = data.with_columns(
            [
                pl.col(i).fill_null(pl.mean(i))
                for i in self.config.dynamic_feature_columns
            ],
        )

        pre_stack = []
        for _, group in data.group_by(self.config.id_column):
            recent = group.tail(sequence_length)
            recent = recent.to_numpy().reshape(-1, recent.height, recent.width)
            recent = tf.keras.utils.pad_sequences(
                recent,
                maxlen=sequence_length,
                padding="pre",
                dtype="float32",
            )
            pre_stack.append(recent)

        tf_data = tf.concat([np.array(i).astype("float32") for i in pre_stack], axis=0)

        for i in repair_index:
            tf_data = self.repair_dimension_post_padding(
                data=tf_data,
                index=i,
                sequence_length=sequence_length,
            )

        return tf_data

    def repair_dimension_post_padding(
        self,
        data: tf.Tensor,
        index: int,
        sequence_length: int = 10,
    ) -> tf.Tensor:
        repair_dimension = tf.repeat(
            tf.reduce_max(
                data[:, :, index],
                axis=1,
                keepdims=True,
            ),
            repeats=sequence_length,
            axis=1,
        )
        return tf.constant(tf.Variable(data)[:, :, index].assign(repair_dimension))

    def process_dynamic_features(
        self,
        future_dynamic_features: pl.DataFrame,
        horizon: int = 4,
    ) -> tf.Tensor:
        future_dynamic_features = self.map_id_to_unique_id(
            future_dynamic_features,
            self.config.id_column,
        )

        check_columns = [self.config.id_column, *self.config.dynamic_feature_columns]
        for i in check_columns:
            if i not in future_dynamic_features.columns:
                msg = f"Column {i} not in dataframe"
                raise ValueError(msg)

        return self.process_checked_data(
            future_dynamic_features.select(check_columns),
            sequence_length=horizon,
            dyanmic_only=True,
        )

    def take_most_recent_slice(self, tensor: tf.Tensor) -> tf.Tensor:
        """This does assume the bottom value is the most recent"""
        tensor = tf.reverse(tensor, axis=[1])
        return tf.gather(tensor, indices=[0], axis=1)

    def generate_future_date_spine(self, num_weeks: int) -> pl.DataFrame:
        self.future_dates = []
        for i in range(1, num_weeks + 1):
            if self.config.date_unit == "week":
                self.future_dates.append(
                    str(self.most_recent_date + datetime.timedelta(weeks=i)),
                )
            else:
                self.future_dates.append(
                    str(self.most_recent_date + datetime.timedelta(days=i)),
                )

        combinations = list(
            itertools.product(
                self.categorical_encodings[self.config.id_column]["mapping"].keys(),
                self.future_dates,
            )
        )
        data = pl.DataFrame(
            {
                self.config.id_column: [i[0] for i in combinations],
                self.config.date_column: [i[1] for i in combinations],
            },
        )

        return data.with_columns(pl.col(self.config.date_column).cast(pl.Date)).sort(
            by=self.config.date_column,
            descending=False,
        )

    def build_final_output(self, predictions: tf.Tensor) -> pl.DataFrame:
        output = pl.DataFrame(
            {
                self.config.id_column: np.unique(
                    predictions[:, :, 0].numpy().flatten(),
                ),
                **{
                    self.future_dates[i]: predictions[:, :, 1].numpy()[:, i]
                    for i in range(len(self.future_dates))
                },
            },
        )

        output = output.with_columns(
            pl.col(self.config.id_column).map_elements(
                lambda x: next(
                    k
                    for k, v in self.categorical_encodings[self.config.id_column][
                        "mapping"
                    ].items()
                    if v == x
                ),
                return_dtype=pl.Utf8,
            ),
        )

        return output.unpivot(index=self.config.id_column).rename(
            mapping={"variable": self.config.date_column, "value": "predicted"},
        )

    @property
    def categories_to_embed(self) -> dict[str, int]:
        cardinality_threshold = 5
        return {
            k: v["cardinality"]
            for k, v in self.categorical_encodings.items()
            if v["cardinality"] > cardinality_threshold
        }

    @property
    def train(self) -> tf.Tensor:
        input_slice = slice(
            -self.total_window_size,
            -self.total_window_size + self.config.input_width_weeks,
        )
        train_df = self.data[:, input_slice, :]
        train_df.set_shape([None, self.config.input_width_weeks, None])
        return train_df

    @property
    def val(self) -> tf.Tensor:
        validation_slice = slice(
            -self.total_window_size
            + (self.config.input_width_weeks + self.config.target_width_weeks),
            None if self.config.validation_width_weeks != 0 else 0,
        )
        val_df = self.data[:, validation_slice, :]
        val_df.set_shape([None, self.config.validation_width_weeks, None])
        return val_df

    @property
    def test(self) -> tf.Tensor:
        test_slice = slice(
            -self.total_window_size + self.config.input_width_weeks,
            -self.total_window_size
            + (self.config.input_width_weeks + self.config.target_width_weeks)
            if self.config.validation_width_weeks != 0
            else None,
        )
        test_df = self.data[:, test_slice, :]
        test_df.set_shape([None, self.config.target_width_weeks, None])
        return test_df

    @property
    def last_target_value(self) -> tf.Tensor:
        return self.take_most_recent_slice(
            tf.gather(self.test, indices=[self.target_column_index], axis=2),
        )

    @property
    def static_feature_slice(self) -> tf.Tensor:
        if len(self.static_feature_columns_indices) == 0:
            return tf.zeros_like(self.train)[:, :, 0:0]

        return tf.gather(
            self.take_most_recent_slice(self.train),
            indices=self.static_feature_columns_indices,
            axis=2,
        )

    @property
    def test_dynamic_features(self) -> tf.Tensor:
        return tf.gather(
            self.test,
            indices=self.dynamic_feature_columns_indices,
            axis=2,
        )

    @property
    def val_dynamic_features(self) -> tf.Tensor:
        return tf.gather(self.val, indices=self.dynamic_feature_columns_indices, axis=2)

    @property
    def train_dynamic_features(self) -> tf.Tensor:
        return tf.gather(
            self.train,
            indices=self.dynamic_feature_columns_indices,
            axis=2,
        )

    @property
    def train_target(self) -> tf.Tensor:
        return tf.gather(self.train, indices=[self.target_column_index], axis=2)[
            :,
            :,
            0,
        ]

    @property
    def val_target(self) -> tf.Tensor:
        return tf.gather(self.val, indices=[self.target_column_index], axis=2)[:, :, 0]

    @property
    def test_target(self) -> tf.Tensor:
        return tf.gather(self.test, indices=[self.target_column_index], axis=2)[:, :, 0]
