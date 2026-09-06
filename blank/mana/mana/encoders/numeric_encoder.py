from pathlib import Path

import joblib
import numpy as np

from mana.encoders.encoder_protocol import EncoderProtocol


class NumericEncoder(EncoderProtocol):
    """
    NumericEncoder normalizes numeric features using min-max scaling.
    """

    def __init__(
        self,
        name: str = "NumericEncoder",
        column_key: str | None = None,
    ):
        """
        Initialize the NumericEncoder.

        Parameters:
            name (str): Name of the encoder.
            column_key (str | None): Optional key to identify the column being encoded.
        """
        self.min_ = None
        self.max_ = None
        self.name = name
        self.type = "NumericEncoder"
        self.column_key = column_key
        self.has_been_fitted = False

    def fit(self, data: list) -> None:
        """
        Fit the encoder to the data.

        Parameters:
            data (array-like): Numeric feature values.
        """
        data = np.asarray(data)
        self.min_ = np.min(data)
        self.max_ = np.max(data)
        self.has_been_fitted = True

    def transform(self, data: list) -> np.ndarray:
        """
        Transform the data using min-max normalization.

        Parameters:
            data (array-like): Numeric feature values.

        Returns:
            np.ndarray: Normalized values in [0, 1].
        """
        data = np.asarray(data)
        if self.min_ is None or self.max_ is None:
            raise ValueError("Encoder has not been fitted yet.")
        # Avoid division by zero
        if self.max_ == self.min_:
            return np.zeros_like(data, dtype=float)
        return (data - self.min_) / (self.max_ - self.min_)

    def inverse_transform(self, data: list | np.ndarray) -> np.ndarray:
        """
        Inverse transform normalized data back to original scale.

        Parameters:
            data (array-like): Normalized values in [0, 1].

        Returns:
            np.ndarray: Original scale values.
        """
        data = np.asarray(data)
        if self.min_ is None or self.max_ is None:
            raise ValueError("Encoder has not been fitted yet.")
        return data * (self.max_ - self.min_) + self.min_

    def fit_transform(self, data: list) -> np.ndarray:
        """
        Fit to data, then transform it.

        Parameters:
            data (array-like): Numeric feature values.

        Returns:
            np.ndarray: Normalized values in [0, 1].
        """
        self.fit(data)
        return self.transform(data)

    def save_encoder(self, filepath: str | Path):
        if isinstance(filepath, str):
            filepath = Path(filepath)

        with filepath.open("wb") as f:
            joblib.dump(self, f)

    def load_encoder(self, filepath: str | Path):
        if isinstance(filepath, str):
            filepath = Path(filepath)

        with filepath.open("rb") as f:
            loaded_encoder = joblib.load(f)

        self.min_ = loaded_encoder.min_
        self.max_ = loaded_encoder.max_
        self.name = loaded_encoder.name
        self.type = loaded_encoder.type
        self.column_key = loaded_encoder.column_key
        self.has_been_fitted = loaded_encoder.has_been_fitted
