"""Phase 1: collect -> train world model -> rollout fidelity report (M1 acceptance)."""

import argparse
import pathlib

import torch

from wilt.config import EnvConfig, ModelConfig, TrainConfig
from wilt.eval.fidelity import fidelity_report, print_report
from wilt.world_model.train import train_world_model


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--episodes", type=int, default=224)
    p.add_argument("--val-episodes", type=int, default=36)
    p.add_argument("--epochs", type=int, default=40)
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--device", default="cpu")
    p.add_argument("--warmup", type=int, default=14)
    p.add_argument("--samples", type=int, default=64)
    args = p.parse_args()

    env_cfg = EnvConfig()
    m_cfg = ModelConfig()
    t_cfg = TrainConfig(
        episodes=args.episodes,
        val_episodes=args.val_episodes,
        epochs=args.epochs,
        seed=args.seed,
        device=args.device,
    )

    model, train_data, val_data = train_world_model(env_cfg, m_cfg, t_cfg)
    report = fidelity_report(
        model, env_cfg, train_data, val_data, warmup=args.warmup, n_samples=args.samples
    )
    print_report(report, warmup=args.warmup)

    out_dir = pathlib.Path(__file__).resolve().parent.parent / "runs"
    out_dir.mkdir(exist_ok=True)
    ckpt = out_dir / "world_model.pt"
    torch.save(model.state_dict(), ckpt)
    print(f"\ncheckpoint saved to {ckpt}")


if __name__ == "__main__":
    main()
