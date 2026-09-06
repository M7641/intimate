"""Phase 1 acceptance: does the learned model's imagination match reality?

Protocol per held-out episode:
  1. warm up the posterior for `warmup` steps (teacher forcing, true actions),
  2. open-loop the rest: prior z, true actions, NO access to realised demand,
  3. compare against the simulator.

Because the env is synthetic we have the ORACLE demand mean, so model error is
measured against the true mean rather than against irreducibly noisy realisations.
Inventory fidelity pushes sampled demand through the SAME analytic accounting used
by the env, starting from the true physical state at the warmup boundary.
"""

import torch

from wilt.config import EnvConfig
from wilt.env.accounting import reward_torch, step_inventory_torch


def smape(pred: torch.Tensor, target: torch.Tensor) -> torch.Tensor:
    return 2.0 * (pred - target).abs() / (pred.abs() + target.abs() + 1e-6)


def dow_mean_baseline(train_data: dict) -> torch.Tensor:
    """Seasonal-naive: average training demand per day-of-week."""
    demand, dow = train_data["demand"], train_data["dow"]
    means = torch.zeros(7)
    for d in range(7):
        means[d] = demand[dow == d].mean()
    return means


@torch.no_grad()
def fidelity_report(
    model,
    env_cfg: EnvConfig,
    train_data: dict,
    val_data: dict,
    warmup: int = 14,
    n_samples: int = 64,
) -> dict:
    W, K = warmup, n_samples
    B, T = val_data["demand"].shape
    H = T - W

    h = model.warmup(val_data, W)
    out = model.open_loop(h, val_data, W, K)

    pred_mean = out["mean"].mean(dim=1)  # (B, H) marginal mean
    oracle_mean = val_data["demand_mean"][:, W:]
    naive = dow_mean_baseline(train_data)[val_data["dow"][:, W:]]

    err_model = smape(pred_mean, oracle_mean)
    err_naive = smape(naive, oracle_mean)

    def bucket(err: torch.Tensor, k: int) -> float:
        return float(err[:, :k].mean())

    lo = out["sample"].float().quantile(0.1, dim=1)
    hi = out["sample"].float().quantile(0.9, dim=1)
    realised = val_data["demand"][:, W:]
    coverage = float(((realised >= lo) & (realised <= hi)).float().mean())

    # --- inventory/profit fidelity: sampled demand through exact accounting -----
    q = val_data["q"][:, W].repeat_interleave(K, dim=0)
    o = val_data["o"][:, W].repeat_interleave(K, dim=0)
    demand_s = out["sample"].reshape(B * K, H)
    profit = torch.zeros(B * K)
    waste = torch.zeros(B * K)
    sold = torch.zeros(B * K)
    for t in range(H):
        order = val_data["order_qty"][:, W + t].repeat_interleave(K, dim=0)
        price = val_data["price"][:, W + t].repeat_interleave(K, dim=0)
        q, o, flows = step_inventory_torch(q, o, order, demand_s[:, t])
        profit += reward_torch(env_cfg, price, order, flows)
        waste += flows.spoiled
        sold += flows.sales
    profit += env_cfg.salvage_value * q.sum(dim=1)  # terminal salvage, as in the env

    true_profit = val_data["reward"][:, W:].sum(dim=1)
    true_waste = val_data["spoiled"][:, W:].sum(dim=1)
    true_sold = val_data["sales"][:, W:].sum(dim=1)

    def rel_err(pred: torch.Tensor, true: torch.Tensor) -> float:
        return float((pred.view(B, K).mean(1) - true).abs().mean() / true.abs().mean())

    report = {
        "smape_oracle_k1": bucket(err_model, 1),
        "smape_oracle_k7": bucket(err_model, 7),
        "smape_oracle_k14": bucket(err_model, 14),
        "smape_oracle_all": bucket(err_model, H),
        "smape_naive_k1": bucket(err_naive, 1),
        "smape_naive_k7": bucket(err_naive, 7),
        "smape_naive_all": bucket(err_naive, H),
        "coverage_80": coverage,
        "profit_rel_err": rel_err(profit, true_profit),
        "waste_rel_err": rel_err(waste, true_waste),
        "sales_rel_err": rel_err(sold, true_sold),
    }
    return report


def print_report(report: dict, warmup: int) -> None:
    r = report
    print(
        f"\n=== rollout fidelity (open loop after {warmup}-step posterior warmup) ==="
    )
    print(f"{'horizon':<12}{'model sMAPE vs oracle':>24}{'seasonal-naive':>18}")
    print(f"{'k=1':<12}{r['smape_oracle_k1']:>24.3f}{r['smape_naive_k1']:>18.3f}")
    print(f"{'k<=7':<12}{r['smape_oracle_k7']:>24.3f}{r['smape_naive_k7']:>18.3f}")
    print(f"{'k<=14':<12}{r['smape_oracle_k14']:>24.3f}{'-':>18}")
    print(f"{'all':<12}{r['smape_oracle_all']:>24.3f}{r['smape_naive_all']:>18.3f}")
    print(
        f"\n80% interval coverage of realised demand: {r['coverage_80']:.3f}  (target ~0.80)"
    )
    print("imagined accounting vs realised episode (relative error of the mean):")
    print(
        f"  cumulative profit {r['profit_rel_err']:.3f}   "
        f"waste {r['waste_rel_err']:.3f}   sales {r['sales_rel_err']:.3f}"
    )
