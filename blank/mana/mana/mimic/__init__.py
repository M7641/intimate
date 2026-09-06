"""Mimic: Contrastive learning library for set-structured data."""

from .aggregators import MeanAggregator
from .augmentations import (
    Compose,
    CurriculumAugmentation,
    FeatureMask,
    FeatureNoise,
    SubsetSample,
)
from .data import SetBatch, SetDataset, collate_sets, set_dataloader
from .element_encoders import MLPElementEncoder, TabularElementEncoder
from .ema import EMAEncoder
from .encoders import DeepSetsEncoder, SetTransformerEncoder
from .evaluate import alignment, knn_accuracy, linear_probe, silhouette, uniformity
from .losses import (
    NTXentLoss,
    SupConLoss,
    covariance_loss,
    uniformity_loss,
    variance_loss,
)
from .methods import BaseMethod, ContrastiveMethod, MaskedSetAutoencoder
from .model import MimicModel
from .projectors import MLPProjector, make_decoder
from .trainer import ContrastiveTrainer, SSLTrainer, TrainConfig, TrainResult
from .types import Method

__all__ = [
    # Element encoders
    "MLPElementEncoder",
    "TabularElementEncoder",
    # Aggregators
    "MeanAggregator",
    # Encoders
    "DeepSetsEncoder",
    "SetTransformerEncoder",
    # EMA
    "EMAEncoder",
    # Projectors
    "MLPProjector",
    "make_decoder",
    # Losses
    "NTXentLoss",
    "SupConLoss",
    # Regularizers
    "covariance_loss",
    "uniformity_loss",
    "variance_loss",
    # Augmentations
    "Compose",
    "CurriculumAugmentation",
    "FeatureMask",
    "FeatureNoise",
    "SubsetSample",
    # Data
    "SetBatch",
    "SetDataset",
    "collate_sets",
    "set_dataloader",
    # Methods (axis B — objectives)
    "Method",
    "BaseMethod",
    "ContrastiveMethod",
    "MaskedSetAutoencoder",
    # Model (backward-compatible alias for ContrastiveMethod)
    "MimicModel",
    # Training
    "SSLTrainer",
    "ContrastiveTrainer",
    "TrainConfig",
    "TrainResult",
    # Evaluation
    "alignment",
    "knn_accuracy",
    "linear_probe",
    "silhouette",
    "uniformity",
]
