from pathlib import Path
from typing import Protocol


class EncoderProtocol(Protocol):
    name: str
    type: str
    column_key: str | None
    has_been_fitted: bool

    def fit(self, data: list) -> None: ...

    def transform(self, data: list) -> list: ...

    def fit_transform(self, data: list) -> list: ...

    def inverse_transform(self, data: list) -> list: ...

    def save_encoder(self, filepath: str | Path) -> None: ...

    def load_encoder(self, filepath: str | Path) -> None: ...
