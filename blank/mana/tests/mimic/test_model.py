"""Tests for MimicModel."""

import torch
import torch.nn as nn

from mana.mimic import (
    Compose,
    DeepSetsEncoder,
    FeatureNoise,
    MeanAggregator,
    MLPElementEncoder,
    MLPProjector,
    MimicModel,
    NTXentLoss,
    make_decoder,
)

from mana.mimic.data import SetBatch

INPUT_DIM = 16
HIDDEN = 32
OUTPUT = 24
PROJ_OUT = 16


def _make_model(
    input_dim: int = INPUT_DIM,
    hidden: int = HIDDEN,
    output: int = OUTPUT,
    proj_out: int = PROJ_OUT,
    decoder: nn.Module | None = None,
    reconstruction_weight: float = 0.1,
    uniformity_weight: float = 0.0,
    variance_weight: float = 0.0,
    covariance_weight: float = 0.0,
) -> MimicModel:
    return MimicModel(
        encoder=DeepSetsEncoder(
            element_encoder=MLPElementEncoder(input_dim, hidden, hidden),
            aggregator=MeanAggregator(),
            rho=nn.Sequential(nn.Linear(hidden, output), nn.ReLU()),
            output_dim=output,
        ),
        projector=MLPProjector(
            input_dim=output, hidden_dim=hidden, output_dim=proj_out
        ),
        augmentation=Compose([FeatureNoise(std=0.1)]),
        loss_fn=NTXentLoss(temperature=0.1),
        decoder=decoder,
        reconstruction_weight=reconstruction_weight,
        uniformity_weight=uniformity_weight,
        variance_weight=variance_weight,
        covariance_weight=covariance_weight,
    )


class TestMimicModel:
    def test_forward_returns_scalar(self, random_set_batch: SetBatch) -> None:
        model = _make_model()
        model.train()
        loss = model(random_set_batch.x, random_set_batch.mask)
        assert loss.shape == ()
        assert loss.requires_grad

    def test_encode_shape(self, random_set_batch: SetBatch) -> None:
        model = _make_model()
        emb = model.encode(random_set_batch.x, random_set_batch.mask)
        assert emb.shape == (4, 24)
        assert not emb.requires_grad

    def test_embed_shape(self, random_set_batch: SetBatch) -> None:
        model = _make_model()
        emb = model.embed(random_set_batch.x, random_set_batch.mask)
        assert emb.shape == (4, 16)
        assert not emb.requires_grad

    def test_encode_restores_training_mode(self) -> None:
        model = _make_model()
        model.train()
        assert model.training
        x = torch.randn(2, 5, 16)
        model.encode(x)
        assert model.training  # Should be restored

    def test_forward_is_differentiable(self, random_set_batch: SetBatch) -> None:
        model = _make_model()
        model.train()
        loss = model(random_set_batch.x, random_set_batch.mask)
        loss.backward()
        # Check at least one parameter has gradients
        has_grad = any(p.grad is not None for p in model.parameters())
        assert has_grad


class TestReconstruction:
    def test_forward_with_decoder_returns_scalar(
        self, random_set_batch: SetBatch
    ) -> None:
        decoder = make_decoder(OUTPUT, HIDDEN, INPUT_DIM)
        model = _make_model(decoder=decoder)
        model.train()
        loss = model(random_set_batch.x, random_set_batch.mask)
        assert loss.shape == ()
        assert loss.requires_grad

    def test_forward_without_decoder_unchanged(
        self, random_set_batch: SetBatch
    ) -> None:
        torch.manual_seed(0)
        model_no_dec = _make_model()
        model_no_dec.train()
        loss_no_dec = model_no_dec(random_set_batch.x, random_set_batch.mask)

        torch.manual_seed(0)
        model_none = _make_model(decoder=None)
        model_none.load_state_dict(model_no_dec.state_dict(), strict=False)
        model_none.train()
        loss_none = model_none(random_set_batch.x, random_set_batch.mask)

        assert torch.isclose(loss_no_dec, loss_none)

    def test_reconstruction_weight_zero_equals_contrastive_only(
        self, random_set_batch: SetBatch
    ) -> None:
        # Build baseline model without decoder
        torch.manual_seed(7)
        model_no_dec = _make_model()
        model_no_dec.train()

        # Build model with decoder and weight=0, copy shared weights from baseline
        decoder = make_decoder(OUTPUT, HIDDEN, INPUT_DIM)
        model = _make_model(decoder=decoder, reconstruction_weight=0.0)
        shared = {k: v for k, v in model_no_dec.state_dict().items()}
        model.load_state_dict(shared, strict=False)
        model.train()

        # Same seed for augmentation randomness
        torch.manual_seed(42)
        loss_no_dec = model_no_dec(random_set_batch.x, random_set_batch.mask)
        torch.manual_seed(42)
        loss_with_dec = model(random_set_batch.x, random_set_batch.mask)

        assert torch.isclose(loss_with_dec, loss_no_dec)

    def test_decoder_receives_gradients(self, random_set_batch: SetBatch) -> None:
        decoder = make_decoder(OUTPUT, HIDDEN, INPUT_DIM)
        model = _make_model(decoder=decoder)
        model.train()
        loss = model(random_set_batch.x, random_set_batch.mask)
        loss.backward()
        for p in decoder.parameters():
            assert p.grad is not None

    def test_encode_ignores_decoder(self, random_set_batch: SetBatch) -> None:
        decoder = make_decoder(OUTPUT, HIDDEN, INPUT_DIM)
        model = _make_model(decoder=decoder)
        emb = model.encode(random_set_batch.x, random_set_batch.mask)
        assert emb.shape == (4, OUTPUT)
        assert not emb.requires_grad

    def test_reconstruction_weight_scales_loss(
        self, random_set_batch: SetBatch
    ) -> None:
        torch.manual_seed(99)
        decoder = make_decoder(OUTPUT, HIDDEN, INPUT_DIM)
        model_low = _make_model(decoder=decoder, reconstruction_weight=0.01)
        model_low.train()
        loss_low = model_low(random_set_batch.x, random_set_batch.mask)

        torch.manual_seed(99)
        decoder_high = make_decoder(OUTPUT, HIDDEN, INPUT_DIM)
        model_high = _make_model(decoder=decoder_high, reconstruction_weight=10.0)
        # Copy all shared weights so only the weight differs
        model_high.load_state_dict(model_low.state_dict())
        model_high.reconstruction_weight = 10.0
        model_high.train()
        loss_high = model_high(random_set_batch.x, random_set_batch.mask)

        assert loss_high > loss_low


class TestRegularizers:
    def test_all_regularizers_differentiable(self, random_set_batch: SetBatch) -> None:
        model = _make_model(
            uniformity_weight=0.1,
            variance_weight=0.1,
            covariance_weight=0.04,
        )
        model.train()
        loss = model(random_set_batch.x, random_set_batch.mask)
        assert loss.shape == ()
        loss.backward()
        has_grad = any(p.grad is not None for p in model.parameters())
        assert has_grad

    def test_uniformity_weight_zero_no_change(self, random_set_batch: SetBatch) -> None:
        torch.manual_seed(42)
        model_base = _make_model()
        model_base.train()

        torch.manual_seed(42)
        model_reg = _make_model(
            uniformity_weight=0.0,
            variance_weight=0.0,
            covariance_weight=0.0,
        )
        model_reg.load_state_dict(model_base.state_dict())
        model_reg.train()

        torch.manual_seed(99)
        loss_base = model_base(random_set_batch.x, random_set_batch.mask)
        torch.manual_seed(99)
        loss_reg = model_reg(random_set_batch.x, random_set_batch.mask)

        assert torch.isclose(loss_base, loss_reg)
