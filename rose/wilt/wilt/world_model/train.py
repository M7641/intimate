"""World-model training: collect episodes, fit by posterior filtering, track val NLL."""

import numpy as np
import torch

from wilt.config import EnvConfig, ModelConfig, TrainConfig
from wilt.data.buffer import episodes_to_tensors, iter_batches
from wilt.data.collect import collect_episodes
from wilt.world_model.model import WorldModel


def train_world_model(
    env_cfg: EnvConfig, m_cfg: ModelConfig, t_cfg: TrainConfig
) -> tuple[WorldModel, dict, dict]:
    torch.manual_seed(t_cfg.seed)
    rng = np.random.default_rng(t_cfg.seed)

    episodes = collect_episodes(
        env_cfg, t_cfg.episodes + t_cfg.val_episodes, seed=t_cfg.seed
    )
    train_data = episodes_to_tensors(episodes[: t_cfg.episodes], env_cfg, t_cfg.device)
    val_data = episodes_to_tensors(episodes[t_cfg.episodes :], env_cfg, t_cfg.device)
    print(
        f"collected {t_cfg.episodes} train / {t_cfg.val_episodes} val episodes "
        f"of {env_cfg.horizon} steps"
    )

    model = WorldModel(env_cfg, m_cfg).to(t_cfg.device)
    opt = torch.optim.Adam(model.parameters(), lr=t_cfg.lr)

    for epoch in range(1, t_cfg.epochs + 1):
        model.train()
        ep_metrics: dict[str, list] = {"loss": [], "nll": [], "kl": []}
        for batch in iter_batches(train_data, t_cfg.batch_size, rng):
            loss, metrics = model.observe(batch)
            opt.zero_grad()
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), t_cfg.grad_clip)
            opt.step()
            ep_metrics["loss"].append(float(loss.detach()))
            ep_metrics["nll"].append(metrics["nll"])
            ep_metrics["kl"].append(metrics["kl"])

        if epoch % 5 == 0 or epoch == 1:
            model.eval()
            with torch.no_grad():
                _, val_metrics = model.observe(val_data)
            print(
                f"epoch {epoch:3d}  "
                f"train loss {np.mean(ep_metrics['loss']):7.3f}  "
                f"nll {np.mean(ep_metrics['nll']):6.3f}  "
                f"kl {np.mean(ep_metrics['kl']):5.2f}  |  "
                f"val nll {val_metrics['nll']:6.3f}"
            )

    return model, train_data, val_data
