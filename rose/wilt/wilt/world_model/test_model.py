import torch

from wilt.config import EnvConfig, ModelConfig
from wilt.world_model.model import WorldModel


def tiny_batch(B=4, T=10):
    g = torch.Generator().manual_seed(0)
    return {
        "demand": torch.randint(0, 60, (B, T), generator=g).float(),
        "price_norm": torch.rand(B, T, generator=g) * 0.6 + 0.4,
        "order_norm": torch.rand(B, T, generator=g),
        "dow": torch.randint(0, 7, (B, T), generator=g),
    }


def make_model():
    return WorldModel(
        EnvConfig(), ModelConfig(h_dim=32, z_dim=8, hidden_dim=32, embed_dim=16)
    )


def test_observe_loss_finite_and_backpropagates():
    model = make_model()
    loss, metrics = model.observe(tiny_batch())
    assert torch.isfinite(loss)
    loss.backward()
    grads = [p.grad for p in model.parameters() if p.grad is not None]
    assert grads and all(torch.isfinite(g).all() for g in grads)
    assert metrics["kl"] >= 0


def test_open_loop_shapes_and_values():
    model = make_model()
    batch = tiny_batch(B=3, T=12)
    h = model.warmup(batch, steps=5)
    out = model.open_loop(h, batch, start=5, n_samples=7)
    assert out["mean"].shape == (3, 7, 7)
    assert out["sample"].shape == (3, 7, 7)
    assert torch.isfinite(out["mean"]).all()
    assert (out["sample"] >= 0).all()
