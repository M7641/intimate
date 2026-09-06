"""Phase 0 smoke run: exercise the synthetic env with the heuristic behaviour policy.

Prints per-episode economics and checks the conservation invariant end to end.
"""

import numpy as np

from wilt.config import EnvConfig
from wilt.data.collect import collect_episodes


def main() -> None:
    cfg = EnvConfig()
    episodes = collect_episodes(cfg, n_episodes=5, seed=0)
    print(f"{'ep':<4}{'profit':>10}{'demand':>9}{'sales':>8}{'fill%':>8}{'waste%':>8}")
    for i, ep in enumerate(episodes):
        received = cfg.initial_stock + ep.arrived.sum()
        conserved = received == ep.sales.sum() + ep.spoiled.sum() + ep.q[-1].sum()
        assert conserved, f"conservation violated in episode {i}"
        fill = ep.sales.sum() / max(1, ep.demand.sum())
        waste = ep.spoiled.sum() / max(1, received)
        print(
            f"{i:<4}{ep.reward.sum():>10.1f}{ep.demand.sum():>9}{ep.sales.sum():>8}"
            f"{100 * fill:>7.1f}%{100 * waste:>7.1f}%"
        )
    print(
        "\nconservation invariant holds on all episodes "
        "(initial + arrived == sold + spoiled + remaining)"
    )
    d = np.concatenate([ep.demand for ep in episodes])
    print(
        f"demand across episodes: mean {d.mean():.1f}, sd {d.std():.1f}, max {d.max()}"
    )


if __name__ == "__main__":
    main()
