import logging
import os
import secrets
import shutil
import string
import warnings
from pathlib import Path
from typing import Literal

import joblib
import numpy as np
import pandas as pd
from metis.utility_tools.s3_tools import (download_most_recent_tar_file,
                                          upload_to_s3)
from pydantic import BaseModel, Field
from sklearn.preprocessing import OneHotEncoder, OrdinalEncoder, StandardScaler


def id_generator(size=8, chars=string.ascii_uppercase + string.digits):
    return "".join(secrets.SystemRandom().choice(chars) for _ in range(size))


class ManualEncodingChoices(BaseModel):
    one_hot_encoder: list | None = Field(
        None, nullable=True, description="The one hot encoder list."
    )
    ordinal_encoder: list | None = Field(
        None, nullable=True, description="The ordinal encoder list."
    )
    numerical_encoder: list | None = Field(
        None, nullable=True, description="The numerical encoder list."
    )
    date_encoder: list | None = Field(
        None, nullable=True, description="The date encoder list."
    )


class DataStructureSingular(BaseModel):
    date_minimum: str = Field(
        "2000-01-01", description="The minimum date for the data structure."
    )
    manual_encoding_choices: ManualEncodingChoices | None = Field(
        None, nullable=True, description="The manual encoding choices for the encoder."
    )


class DataConstructor:
    """
    The job of this class is to map any data structure to the appropriate format for the model.
    """

    def __init__(self, data_config: dict = {}, save_append_name: str = "") -> None:
        self.data_config = DataStructureSingular(**data_config)
        self.save_append_name = save_append_name

    def process_data_for_model(
        self, data: pd.DataFrame, train_encoders: Literal["train", "use_saved"]
    ) -> pd.DataFrame:
        """
        Processes the given data for the model.

        Parameters:
            data (pd.DataFrame): The data to be processed.
            predict (bool, optional): Whether to perform prediction or not. Defaults to False.

        Returns:
            The processed data.

        Raises:
            None.
        """
        return self._process_single_row_relational(
            data=data, train_encoders=train_encoders
        )

    def _save_pickle(self, file, object_name: str):
        """
        Saves the encoder file to AWS S3.

        Args:
            encoder: The encoder file to be saved.
            encoder_name (str): The name of the encoder file.

        Returns:
            None
        """
        logging.info(f"Saving {object_name} to S3.")
        upload_to_s3(
            file=file,
            model_name="model_explain_" + self.save_append_name,
            object_name=object_name,
        )

    def _load_pickle(
        self,
        object_name: str,
    ):
        """
        Load the encoder from S3.

        Args:
            encoder_name (str): The name of the encoder.

        Returns:
            The loaded encoder object.
        """
        logging.info(f"Loading {object_name} from S3.")
        load_from = str(Path(__file__).parent.resolve()) + "/temp/"

        download_most_recent_tar_file(
            save_dir=str(Path(__file__).parent.resolve()),
            s3_key=os.environ.get("TENANT")
            + "/datascience/models/model_explain_"
            + self.save_append_name
            + "/"
            + object_name,
        )

        encoder = joblib.load(load_from + f"{object_name}.joblib")

        delete_dir = Path(__file__).parent.resolve() / "temp/"
        if delete_dir.is_dir():
            shutil.rmtree(str(delete_dir))

        return encoder

    def _enforce_categorical_values_for_prediction(
        self,
        dataFrame: pd.DataFrame,
        values_encoded_for: list,
    ) -> pd.DataFrame:
        """
        Enforces categorical values for prediction in the provided DataFrame. If the value
        was not seen at train time it is replaced with "unknown". Equivalent to None.

        The copy is purely to get rid of SettingWithCopyWarning's from Pandas.

        Parameters:
            data (pd.DataFrame): The DataFrame to enforce categorical values on.
            values_encoded_for (list): A list of columns to enforce categorical values for.

        Returns:
            pd.DataFrame: A copy of the input DataFrame with categorical values enforced for the specified columns.
        """
        copy_df = dataFrame.copy(deep=True)
        for column in values_encoded_for:
            copy_df.loc[:, column] = np.select(
                [copy_df.loc[:, column].isin(values_encoded_for[column])],
                [copy_df.loc[:, column]],
                "unknown",
            )
        return copy_df

    def _fit_standard_encoder(
        self,
        dataFrame: pd.DataFrame,
        columns: list,
        train_encoders: Literal["train", "use_saved"],
    ) -> pd.DataFrame:
        """
        Fits a standard encoder to the given data.

        Args:
            data (pd.DataFrame): The input data to be encoded.
            columns (list): A list of column names to be encoded.
            predict (bool): Whether to fit the encoder for prediction or training.

        Returns:
            pd.DataFrame: The encoded data.

        """
        for i in columns:
            dataFrame[i] = pd.to_numeric(dataFrame[i])

        if train_encoders == "use_saved":
            scalar_encoder = self._load_pickle(object_name="scalar_encoder_save")
        elif train_encoders == "train":
            values_encoded_for = {}
            for col in columns:
                min_value = dataFrame[col].min()
                max_value = dataFrame[col].max()
                values_encoded_for[col] = [min_value, max_value]

            self._save_pickle(
                file=values_encoded_for,
                object_name="numeric_values_encoded_for_save",
            )

            scalar_encoder = StandardScaler().fit(
                dataFrame[columns],
            )

            self._save_pickle(
                file=scalar_encoder,
                object_name="scalar_encoder_save",
            )
        else:
            raise ValueError("train_encoders must be 'train' or 'use_saved'.")

        dataFrame[columns] = scalar_encoder.transform(dataFrame[columns])
        return dataFrame

    def _fit_one_hot_encoder(
        self,
        dataFrame: pd.DataFrame,
        columns,
        train_encoders: Literal["train", "use_saved"],
    ) -> pd.DataFrame:
        """
        Fits a one-hot encoder on the given data using the specified columns.

        Args:
            data (pd.DataFrame): The input data.
            columns (list): The columns to encode.
            train_encoders (str): Flag indicating whether the encoder is being used for prediction.

        Returns:
            pd.DataFrame: The data with the specified columns encoded and dropped.
        """
        logging.info(f"Encoding {columns} with one hot encoder.")
        if train_encoders == "use_saved":
            one_hot_encoder = self._load_pickle(object_name="one_hot_encoder_save")
            values_encoded_for = self._load_pickle(
                object_name="one_hot_values_encoded_for_save"
            )
        elif train_encoders == "train":
            values_encoded_for = {col: list(dataFrame[col].unique()) for col in columns}
            self._save_pickle(
                file=values_encoded_for, object_name="one_hot_values_encoded_for_save"
            )
            one_hot_encoder = OneHotEncoder(
                handle_unknown="ignore",
                dtype=np.int16,
                sparse_output=False,
            ).fit(dataFrame[columns])

            self._save_pickle(file=one_hot_encoder, object_name="one_hot_encoder_save")
        else:
            raise ValueError("train_encoders must be 'train' or 'use_saved'.")

        logging.info(f"values_encoded_for: {values_encoded_for}.")
        dataFrame = self._enforce_categorical_values_for_prediction(
            dataFrame, values_encoded_for
        )

        one_hot_encoded = pd.DataFrame(
            one_hot_encoder.transform(dataFrame[columns]),
            columns=one_hot_encoder.get_feature_names_out(),
            index=dataFrame.index,
        )

        dataFrame = pd.concat([dataFrame, one_hot_encoded], axis=1)
        dataFrame = dataFrame.drop(columns=columns)

        return dataFrame

    def _fit_ordinal_encoder(
        self,
        dataFrame: pd.DataFrame,
        columns: list,
        train_encoders: Literal["train", "use_saved"],
    ) -> pd.DataFrame:
        """
        Fits an ordinal encoder to the data and performs encoding on the specified columns.

        Args:
            data (pd.DataFrame): The input DataFrame.
            columns (list): The list of column names to perform encoding on.
            train_encoders (str): Whether the function is being used for prediction or training.

        Returns:
            pd.DataFrame: The input DataFrame with the specified columns encoded.
        """
        logging.info(f"Encoding {columns} with ordinal encoder.")
        if train_encoders == "use_saved":
            ordinal_encoder = self._load_pickle(object_name="ordinal_encoder_save")
            values_encoded_for = self._load_pickle(
                object_name="ordinal_values_encoded_for_save"
            )
        elif train_encoders == "train":
            values_encoded_for = {col: list(dataFrame[col].unique()) for col in columns}
            self._save_pickle(
                file=values_encoded_for, object_name="ordinal_values_encoded_for_save"
            )

            ordinal_encoder = OrdinalEncoder(
                handle_unknown="use_encoded_value",
                unknown_value=-1,
                dtype=np.int16,
            ).fit(dataFrame[columns])

            self._save_pickle(file=ordinal_encoder, object_name="ordinal_encoder_save")
        else:
            raise ValueError("train_encoders must be 'train' or 'use_saved'.")

        logging.info(f"values_encoded_for: {values_encoded_for}.")
        dataFrame = self._enforce_categorical_values_for_prediction(
            dataFrame, values_encoded_for
        )
        dataFrame[columns] = ordinal_encoder.transform(dataFrame[columns])

        return dataFrame

    def _fit_date_encoder(
        self,
        dataFrame: pd.DataFrame,
        date_columns: list,
    ) -> pd.DataFrame:
        logging.info(f"fitting {date_columns} with date encoder.")
        min_date = pd.to_datetime(self.data_config.date_minimum)
        for col in date_columns:
            dataFrame[col] = (dataFrame[col] - min_date) / (
                dataFrame[col].max() - min_date
            )

        return dataFrame

    def _return_columns_cardinality(
        self, data: pd.DataFrame, categorical_columns: list
    ) -> pd.DataFrame:
        """
        Return the cardinality of categorical columns in a DataFrame.

        Args:
            data (pd.DataFrame): The input DataFrame.
            categorical_columns (list): A list of categorical column names.

        Returns:
            pd.DataFrame: A DataFrame containing the cardinality of each categorical column, with the column names
                in the "index" column and the cardinality in the "cardinality" column.
        """
        return data[categorical_columns].nunique().reset_index(name="cardinality")

    def infer_date_columns(self, dataFrame: pd.DataFrame) -> list:
        """
        Returns a list of date columns in the DataFrame.

        Args:
            dataFrame (pd.DataFrame): The input DataFrame.

        Returns:
            list: A list of date columns in the DataFrame.

        Note: The string casting is required as pd.to_datetime(numeric) will work and all the numeric
        values will be listed as dates.
        """
        date_columns = []
        for col in dataFrame.columns:
            try:
                pd.to_datetime(
                    dataFrame[col].dropna().astype(str).iloc[0],
                    format="%Y-%m-%d",
                    exact=True,
                )
                date_columns.append(col)
            except (ValueError, TypeError, IndexError):
                continue
        return date_columns

    def infer_numeric_columns(self, dataFrame: pd.DataFrame) -> list:
        "https://pandas.pydata.org/docs/reference/api/pandas.api.types.infer_dtype.html"
        df_types = (
            pd.DataFrame(dataFrame.apply(pd.api.types.infer_dtype, axis=0))
            .reset_index()
            .rename(columns={"index": "column", 0: "type"})
        )
        numeric_columns = df_types.query(
            "type in ['decimal', 'integer', 'floating', 'mixed-integer-float', 'mixed-integer']",
        )["column"].to_list()
        return numeric_columns

    def infer_categorical_columns(self, dataFrame: pd.DataFrame) -> list:
        df_types = (
            pd.DataFrame(dataFrame.apply(pd.api.types.infer_dtype, axis=0))
            .reset_index()
            .rename(columns={"index": "column", 0: "type"})
        )
        categorical_columns = df_types.query(
            "type in ['string', 'unicode', 'boolean', 'categorical']",
        )["column"].to_list()

        return categorical_columns

    def infer_low_cardinality_categorical_columns(
        self, dataFrame: pd.DataFrame
    ) -> list:
        categorical_columns = self.infer_categorical_columns(dataFrame)
        cardinality = self._return_columns_cardinality(dataFrame, categorical_columns)
        low_cardinality = cardinality.loc[cardinality["cardinality"] <= 0][
            "index"
        ].to_list()
        return low_cardinality

    def infer_high_cardinality_categorical_columns(
        self, dataFrame: pd.DataFrame
    ) -> list:
        categorical_columns = self.infer_categorical_columns(dataFrame)
        cardinality = self._return_columns_cardinality(dataFrame, categorical_columns)

        if sum(cardinality["cardinality"] > 100) > 0:
            logging.warning("There are categorical columns with cardinality > 100.")

        high_cardinality = cardinality.loc[cardinality["cardinality"] > 0][
            "index"
        ].to_list()
        return high_cardinality

    def fill_nans(
        self, data, date_columns, numeric_columns, low_cardinality, high_cardinality
    ):
        for col in numeric_columns:
            if data[col].isnull().all():
                data[col] = data[col].fillna(0)
            else:
                data[col] = data[col].fillna(data[col].mean())

        min_date = pd.to_datetime(
            self.data_config.date_minimum, format="%Y-%m-%d", errors="coerce"
        )
        data[date_columns] = data[date_columns].fillna(min_date)

        data[low_cardinality] = data[low_cardinality].fillna("unknown")
        data[high_cardinality] = data[high_cardinality].fillna("unknown")

        return data

    def _process_single_row_relational(
        self,
        data: pd.DataFrame,
        train_encoders: Literal["train", "use_saved"],
    ) -> pd.DataFrame:
        """
        Process a single row of relational data.

        Args:
            data (pd.DataFrame): The input data.
            predict (bool): Whether to perform prediction or not.

        Returns:
            pd.DataFrame: The processed data.
        """
        logging.warning(
            f"""
            Running with train_encoders = {train_encoders}
            and manual_encoding_choices = {self.data_config.manual_encoding_choices}
        """
        )

        if self.data_config.manual_encoding_choices is not None:
            date_columns = self.data_config.manual_encoding_choices.date_encoder
            numeric_columns = self.data_config.manual_encoding_choices.numerical_encoder
            high_cardinality = self.data_config.manual_encoding_choices.ordinal_encoder
            low_cardinality = self.data_config.manual_encoding_choices.one_hot_encoder

        elif (
            self.data_config.manual_encoding_choices is None
            and train_encoders == "train"
        ):
            logging.info("infering encodings...")
            date_columns = self.infer_date_columns(dataFrame=data)
            # Dropped date_columns as that can get included in the other inferences easily.
            numeric_columns = self.infer_numeric_columns(
                dataFrame=data.drop(columns=date_columns)
            )
            high_cardinality = self.infer_high_cardinality_categorical_columns(
                dataFrame=data.drop(columns=date_columns)
            )
            low_cardinality = self.infer_low_cardinality_categorical_columns(
                dataFrame=data.drop(columns=date_columns)
            )
            inferred_encodings = {
                "date_columns": date_columns,
                "numeric_columns": numeric_columns,
                "high_cardinality": high_cardinality,
                "low_cardinality": low_cardinality,
            }
            logging.info(f"saving infered encodings: {inferred_encodings}")
            self._save_pickle(
                file=inferred_encodings, object_name="infered_encodings_save"
            )

        elif (
            self.data_config.manual_encoding_choices is None
            and train_encoders == "use_saved"
        ):
            logging.info("loading infered encodings...")
            inferred_encodings = self._load_pickle(object_name="infered_encodings_save")
            date_columns = inferred_encodings["date_columns"]
            numeric_columns = inferred_encodings["numeric_columns"]
            high_cardinality = inferred_encodings["high_cardinality"]
            low_cardinality = inferred_encodings["low_cardinality"]
            logging.info(f"loaded infered encodings: {inferred_encodings}")

        if date_columns is None:
            date_columns = []
        if numeric_columns is None:
            numeric_columns = []
        if high_cardinality is None:
            high_cardinality = []
        if low_cardinality is None:
            low_cardinality = []

        data[numeric_columns] = data[numeric_columns].astype(float)
        data[low_cardinality + high_cardinality + date_columns] = data[
            low_cardinality + high_cardinality + date_columns
        ].astype(str)

        for col in date_columns:
            data[col] = pd.to_datetime(data[col], errors="coerce")

        all_columns = (
            numeric_columns + low_cardinality + high_cardinality + date_columns
        )
        previous_columns = data.columns.tolist()
        data = data.drop(columns=[i for i in data.columns if i not in all_columns])

        if len(previous_columns) != len(data.columns.to_list()):
            msg = f"columns mismatch: {previous_columns} vs {data.columns}"
            logging.warning(msg)

        logging.info("appending a fake row to ensure None values are encoded for.")
        unique_index = str(data.index[0]) + id_generator()
        fake_row = pd.DataFrame(
            [np.repeat(None, len(data.columns))],
            columns=data.columns,
            index=[unique_index],
        )
        with warnings.catch_warnings():
            warnings.simplefilter(action="ignore", category=FutureWarning)
            data = pd.concat([data, fake_row], axis=0)

        logging.info("filling nans...")
        data = self.fill_nans(
            data,
            date_columns,
            numeric_columns,
            low_cardinality,
            high_cardinality,
        )

        if date_columns:
            logging.info(f"Date columns: {date_columns}")
            data = self._fit_date_encoder(data, date_columns)

        if high_cardinality:
            logging.info(f"High cardinality columns: {high_cardinality}")
            data = self._fit_ordinal_encoder(data, high_cardinality, train_encoders)

        if low_cardinality:
            logging.info(f"Low cardinality columns: {low_cardinality}")
            data = self._fit_one_hot_encoder(data, low_cardinality, train_encoders)

        if numeric_columns:
            logging.info(f"Numeric columns: {low_cardinality}")
            data = self._fit_standard_encoder(data, numeric_columns, train_encoders)

        if train_encoders == "train":
            data_frame_info = {
                "shape": data.shape,
                "data_columns": data.columns.tolist(),
                "sample_row": data.iloc[0].to_dict(),
            }
            logging.info(f"saving data frame info: {data_frame_info}")
            self._save_pickle(file=data_frame_info, object_name="data_frame_info_save")
        else:
            data_frame_info = self._load_pickle(object_name="data_frame_info_save")
            logging.info(f"loaded data frame info: {data_frame_info}")
            if data.columns.tolist() != data_frame_info["data_columns"]:
                msg = f"data columns mismatch: {data.columns.tolist()} vs {data_frame_info['data_columns']}"
                raise ValueError(msg)

        logging.info("dropping fake row...")
        data = data.drop([unique_index])
        return data
