"""Configuration for the environment, world model and training.

All quantities are per period (one period = one day). The horizon is much longer
than shelf_life + lead_time so the policy problem is genuinely sequential.
"""

from dataclasses import dataclass


@dataclass(frozen=True)
class EnvConfig:
    # physical structure
    shelf_life: int = 5  # cohorts of remaining life; fresh arrivals enter at index m-1
    lead_time: int = 2  # periods between placing an order and its arrival
    horizon: int = 60
    initial_stock: int = 30  # fresh units on hand at reset

    # action grids
    base_price: float = 5.0
    price_ladder: tuple[float, ...] = (
        1.0,
        0.85,
        0.7,
        0.55,
        0.4,
    )  # multipliers of base_price
    case_pack: int = 10
    max_cases: int = 12

    # economics (the analytic reward identity)
    unit_cost: float = 2.0
    fixed_order_cost: float = 5.0
    holding_cost: float = 0.02
    waste_cost: float = 0.5
    stockout_penalty: float = 0.5
    salvage_value: float = 0.5

    # ground-truth demand process (what the world model must learn)
    base_demand: float = 30.0
    dow_factors: tuple[float, ...] = (0.9, 0.85, 0.95, 1.0, 1.1, 1.35, 1.25)
    elasticity: float = 1.8  # demand ∝ (price / base_price) ** (-elasticity)
    regime_rho: float = 0.95  # AR(1) on log-demand regime
    regime_sigma: float = 0.08
    nb_dispersion: float = 12.0  # NegBin r; variance = mu + mu^2 / r

    @property
    def prices(self) -> tuple[float, ...]:
        return tuple(m * self.base_price for m in self.price_ladder)

    @property
    def order_choices(self) -> tuple[int, ...]:
        return tuple(c * self.case_pack for c in range(self.max_cases + 1))

    @property
    def max_order(self) -> int:
        return self.case_pack * self.max_cases


@dataclass(frozen=True)
class ModelConfig:
    h_dim: int = 128  # deterministic GRU state
    z_dim: int = 16  # stochastic Gaussian latent
    hidden_dim: int = 128
    embed_dim: int = 32  # observation embedding e_t
    dow_dim: int = 4
    min_std: float = 0.1
    free_bits: float = 1.0  # nats; floor on the KL so the prior stays usable
    kl_balance: float = 0.8  # DreamerV2-style: weight on training the prior


@dataclass(frozen=True)
class TrainConfig:
    episodes: int = 224
    val_episodes: int = 36
    epochs: int = 40
    batch_size: int = 16
    lr: float = 3e-4
    grad_clip: float = 10.0
    seed: int = 0
    device: str = "cpu"
