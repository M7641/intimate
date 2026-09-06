from pathlib import Path

import joblib
import torch
from pure.logging import NimbusLogger
from transformers import AutoModel, AutoTokenizer

from mana.encoders.encoder_protocol import EncoderProtocol

logger = NimbusLogger.get_logger(__name__)


class TextEncoder(EncoderProtocol):
    def __init__(
        self,
        name: str = "TextEncoder",
        column_key: str | None = None,
    ):
        self.name = name
        self.tokenizer = AutoTokenizer.from_pretrained(
            "sentence-transformers/all-MiniLM-L6-v2"
        )
        self.model = AutoModel.from_pretrained("sentence-transformers/all-MiniLM-L6-v2")
        self.type = "TextEncoder"
        self.column_key = column_key
        self.has_been_fitted = True

    def mean_pooling(self, model_output, attention_mask):
        """
        Mean Pooling - Take attention mask into account for correct averaging
        """
        token_embeddings = model_output[
            0
        ]  # First element of model_output contains all token embeddings
        input_mask_expanded = (
            attention_mask.unsqueeze(-1).expand(token_embeddings.size()).float()
        )
        return torch.sum(token_embeddings * input_mask_expanded, 1) / torch.clamp(
            input_mask_expanded.sum(1), min=1e-9
        )

    def transform(self, data: list[str]) -> list[torch.Tensor]:
        encoded_input = self.tokenizer(
            data, padding=True, truncation=True, return_tensors="pt"
        )
        with torch.no_grad():
            model_output = self.model(**encoded_input)

        sentence_embeddings = self.mean_pooling(
            model_output, encoded_input["attention_mask"]
        )
        sentence_embeddings = torch.nn.functional.normalize(
            sentence_embeddings, p=2, dim=1
        )
        sentence_embeddings = torch.nn.functional.avg_pool1d(
            sentence_embeddings.unsqueeze(1), kernel_size=24
        ).squeeze(1)

        return sentence_embeddings

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
            self.name = loaded_encoder.name
            self.type = loaded_encoder.type
            self.column_key = loaded_encoder.column_key
            self.has_been_fitted = loaded_encoder.has_been_fitted

    def fit(self, data: list) -> None:
        logger.warning("Fit is not required for TextEncoder. No action taken.")
        pass

    def fit_transform(self, data: list) -> list[torch.Tensor]:
        self.fit(data)
        return self.transform(data)

    def inverse_transform(self, data: list) -> list:
        logger.warning(
            "Inverse transform is not supported for TextEncoder. Returning input data as is."
        )
        return data
