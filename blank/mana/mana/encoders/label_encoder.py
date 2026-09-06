from pathlib import Path

import joblib

from mana.encoders.encoder_protocol import EncoderProtocol


class LabelEncoder(EncoderProtocol):
    def __init__(
        self,
        name: str = "LabelEncoder",
        column_key: str | None = None,
        normalise: bool = False,
    ):
        """
        A simple label encoder that maps categorical labels to integer indices.

        Args:
            name (str): Name of the encoder.
            object_key (str | None): Optional key to identify the column being encoded.
            normalise (bool): Whether to normalise indices to [0, 1].
        """
        self.label_to_index = {}
        self.index_to_label = {}
        self.name = name
        self.normalise = normalise
        self.type = "LabelEncoder"
        self.column_key = column_key
        self.has_been_fitted = False

    def fit(self, data: list):
        unique_labels = set(data)
        if self.normalise and len(unique_labels) > 1:
            number_of_labels = len(unique_labels)

            self.label_to_index = {
                label: idx / (number_of_labels - 1)
                for idx, label in enumerate(unique_labels)
            }
        else:
            self.label_to_index = {
                label: idx for idx, label in enumerate(unique_labels)
            }

        self.index_to_label = {idx: label for label, idx in self.label_to_index.items()}
        self.has_been_fitted = True

    def transform(self, data: list):
        return [self.label_to_index[label] for label in data]

    def decode(self, item: int | list):
        if isinstance(item, list):
            return [self.decode(subitem) for subitem in item]

        return self.index_to_label[item]

    def inverse_transform(self, data: list | list[list]) -> list:
        return self.decode(data)

    def fit_transform(self, data: list):
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
            self.label_to_index = loaded_encoder.label_to_index
            self.index_to_label = loaded_encoder.index_to_label
            self.name = loaded_encoder.name
            self.normalise = loaded_encoder.normalise
            self.type = loaded_encoder.type
            self.column_key = loaded_encoder.column_key
            self.has_been_fitted = loaded_encoder.has_been_fitted

    def get_all_labels(self) -> list:
        return list(self.label_to_index.keys())

    def get_all_indices(self) -> list:
        return list(self.index_to_label.keys())
