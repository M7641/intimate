"""Episodes -> batched torch tensors for the world model and fidelity eval."""

import numpy as np
import torch

from wilt.config import EnvConfig
from wilt.data.collect import Episode


def episodes_to_tensors(
    episodes: list[Episode], cfg: EnvConfig, device: str = "cpu"
) -> dict[str, torch.Tensor]:
    def stack(attr: str, dtype=torch.float32) -> torch.Tensor:
        return torch.tensor(
            np.stack([getattr(e, attr) for e in episodes]), dtype=dtype, device=device
        )

    data = {
        "demand": stack("demand"),
        "demand_mean": stack("demand_mean"),
        "price": stack("price"),
        "order_qty": stack("order_qty"),
        "dow": stack("dow", torch.long),
        "reward": stack("reward"),
        "sales": stack("sales"),
        "spoiled": stack("spoiled"),
        "q": stack("q"),
        "o": stack("o"),
    }
    data["price_norm"] = data["price"] / cfg.base_price
    data["order_norm"] = data["order_qty"] / cfg.max_order
    return data


def iter_batches(
    data: dict[str, torch.Tensor], batch_size: int, rng: np.random.Generator
):
    n = data["demand"].shape[0]
    order = rng.permutation(n)
    for start in range(0, n, batch_size):
        idx = torch.tensor(order[start : start + batch_size], dtype=torch.long)
        yield {k: v[idx] for k, v in data.items()}
