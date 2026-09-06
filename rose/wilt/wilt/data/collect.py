"""Behaviour policy and episode collection.

The behaviour policy is a randomised base-stock rule with epsilon-random pricing.
The price randomisation is essential: without exogenous price variation the demand
model cannot identify the elasticity (every price would be confounded with whatever
rule produced it).
"""

from dataclasses import dataclass

import numpy as np

from wilt.config import EnvConfig
from wilt.env.simulator import PerishableRetailEnv


@dataclass
class Episode:
    # per-step arrays, length T
    demand: np.ndarray
    demand_mean: np.ndarray  # ground-truth mean (oracle, for fidelity eval only)
    price_idx: np.ndarray
    price: np.ndarray
    order_idx: np.ndarray
    order_qty: np.ndarray
    dow: np.ndarray
    sales: np.ndarray
    spoiled: np.ndarray
    unmet: np.ndarray
    arrived: np.ndarray
    reward: np.ndarray
    # physical state BEFORE each step (length T+1: includes the final state)
    q: np.ndarray  # (T+1, shelf_life)
    o: np.ndarray  # (T+1, lead_time)


def heuristic_action(
    rng: np.random.Generator,
    cfg: EnvConfig,
    q: np.ndarray,
    o: np.ndarray,
    base_stock: int,
    price_eps: float,
) -> tuple[int, int]:
    position = int(q.sum() + o.sum())
    need = max(0, base_stock - position)
    order_idx = min(cfg.max_cases, -(-need // cfg.case_pack))  # ceil division

    if rng.random() < price_eps:
        price_idx = int(rng.integers(len(cfg.prices)))
    elif q.sum() > 1.3 * base_stock:
        price_idx = 2  # crude markdown under overstock
    else:
        price_idx = 0
    return order_idx, price_idx


def collect_episodes(
    cfg: EnvConfig, n_episodes: int, seed: int, price_eps: float = 0.3
) -> list[Episode]:
    rng = np.random.default_rng(seed)
    env = PerishableRetailEnv(cfg)
    episodes = []
    for _ in range(n_episodes):
        # wide range on purpose: low end starves (stockouts), high end overstocks
        # (spoilage) — the dataset must contain both failure modes
        base_stock = int(rng.integers(40, 221))
        env.reset(seed=int(rng.integers(2**31)))
        T = cfg.horizon
        rec = {
            k: np.zeros(T, dtype=np.int64)
            for k in (
                "demand",
                "price_idx",
                "order_idx",
                "order_qty",
                "dow",
                "sales",
                "spoiled",
                "unmet",
                "arrived",
            )
        }
        rec["demand_mean"] = np.zeros(T)
        rec["price"] = np.zeros(T)
        rec["reward"] = np.zeros(T)
        q_hist = np.zeros((T + 1, cfg.shelf_life), dtype=np.int64)
        o_hist = np.zeros((T + 1, cfg.lead_time), dtype=np.int64)

        for t in range(T):
            q_hist[t] = env.q
            o_hist[t] = env.o
            oi, pi = heuristic_action(rng, cfg, env.q, env.o, base_stock, price_eps)
            _, r, _, _, info = env.step((oi, pi))
            rec["dow"][t] = t % 7
            rec["price_idx"][t] = pi
            rec["order_idx"][t] = oi
            rec["reward"][t] = r
            for k in (
                "demand",
                "demand_mean",
                "price",
                "order_qty",
                "sales",
                "spoiled",
                "unmet",
                "arrived",
            ):
                rec[k][t] = info[k]
        q_hist[T] = env.q
        o_hist[T] = env.o
        episodes.append(Episode(q=q_hist, o=o_hist, **rec))
    return episodes
