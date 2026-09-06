"""RSSM world model with a single generative head: the demand distribution.

Per step t (Dreamer convention, adapted to factored retail dynamics):
  prior      p(z_t | h_t)
  encoder    e_t = enc(log1p(d_t), price_t, dow_t)        # demand-relevant obs only
  posterior  q(z_t | h_t, e_t)
  demand     d_t ~ NegBin(. | h_t, z_t, price_t, dow_t)   # the ONLY generative head
  recurrence h_{t+1} = GRU([z_t, price_t, order_t, dow_t], h_t)

Inventory is deliberately absent from the latent: demand does not depend on it, and
the accounting is exact (env/accounting.py) both in reality and in imagination.
"""

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch.distributions import NegativeBinomial

from wilt.config import EnvConfig, ModelConfig

DEMAND_LOG_SCALE = 3.0  # rough scale of log1p(demand); keeps encoder inputs ~O(1)


def mlp(in_dim: int, hidden: int, out_dim: int) -> nn.Sequential:
    return nn.Sequential(
        nn.Linear(in_dim, hidden),
        nn.ELU(),
        nn.Linear(hidden, hidden),
        nn.ELU(),
        nn.Linear(hidden, out_dim),
    )


def gaussian_kl(m1, s1, m2, s2) -> torch.Tensor:
    return (torch.log(s2 / s1) + (s1**2 + (m1 - m2) ** 2) / (2 * s2**2) - 0.5).sum(-1)


class WorldModel(nn.Module):
    def __init__(self, env_cfg: EnvConfig, cfg: ModelConfig):
        super().__init__()
        self.cfg = cfg
        self.dow_emb = nn.Embedding(7, cfg.dow_dim)
        self.encoder = mlp(2 + cfg.dow_dim, cfg.hidden_dim, cfg.embed_dim)
        self.prior_net = mlp(cfg.h_dim, cfg.hidden_dim, 2 * cfg.z_dim)
        self.post_net = mlp(cfg.h_dim + cfg.embed_dim, cfg.hidden_dim, 2 * cfg.z_dim)
        self.gru = nn.GRUCell(cfg.z_dim + 2 + cfg.dow_dim, cfg.h_dim)
        self.head = mlp(cfg.h_dim + cfg.z_dim + 1 + cfg.dow_dim, cfg.hidden_dim, 2)

    # --- distribution helpers -------------------------------------------------

    def _stats(self, raw: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
        mean, raw_std = raw.chunk(2, dim=-1)
        return mean, F.softplus(raw_std) + self.cfg.min_std

    def prior(self, h: torch.Tensor):
        return self._stats(self.prior_net(h))

    def posterior(self, h: torch.Tensor, e: torch.Tensor):
        return self._stats(self.post_net(torch.cat([h, e], dim=-1)))

    def demand_dist(self, h, z, price_n, dow_e) -> NegativeBinomial:
        out = self.head(torch.cat([h, z, price_n, dow_e], dim=-1))
        log_mu = out[..., 0].clamp(-5.0, 8.0)
        r = F.softplus(out[..., 1]).clamp(1e-2, 1e4)
        # mean = r * exp(logits) = mu; parameterising via logits avoids exp(log_mu)
        return NegativeBinomial(total_count=r, logits=log_mu - torch.log(r))

    def _encode(self, d_t, price_n_t, dow_e_t) -> torch.Tensor:
        x = torch.cat([torch.log1p(d_t) / DEMAND_LOG_SCALE, price_n_t, dow_e_t], dim=-1)
        return self.encoder(x)

    def _recur(self, h, z, price_n_t, order_n_t, dow_e_t) -> torch.Tensor:
        return self.gru(torch.cat([z, price_n_t, order_n_t, dow_e_t], dim=-1), h)

    # --- training -------------------------------------------------------------

    def observe(self, batch: dict) -> tuple[torch.Tensor, dict]:
        """Posterior filtering over full sequences; returns loss = NLL + balanced KL."""
        d = batch["demand"]
        B, T = d.shape
        dow_e = self.dow_emb(batch["dow"])
        h = d.new_zeros(B, self.cfg.h_dim)
        nll_sum = d.new_zeros(B)
        kl_sum = d.new_zeros(B)
        kl_raw_sum = d.new_zeros(B)

        for t in range(T):
            pn = batch["price_norm"][:, t : t + 1]
            on = batch["order_norm"][:, t : t + 1]
            de = dow_e[:, t]

            pm, ps = self.prior(h)
            e = self._encode(d[:, t : t + 1], pn, de)
            qm, qs = self.posterior(h, e)
            z = qm + qs * torch.randn_like(qs)

            nll_sum += -self.demand_dist(h, z, pn, de).log_prob(d[:, t])
            kl_sum += self._balanced_kl(qm, qs, pm, ps)
            kl_raw_sum += gaussian_kl(qm, qs, pm, ps).detach()

            h = self._recur(h, z, pn, on, de)

        loss = (nll_sum + kl_sum).mean() / T
        metrics = {
            "nll": float(nll_sum.detach().mean()) / T,
            "kl": float(kl_raw_sum.mean()) / T,
        }
        return loss, metrics

    def _balanced_kl(self, qm, qs, pm, ps) -> torch.Tensor:
        a, fb = self.cfg.kl_balance, self.cfg.free_bits
        train_prior = gaussian_kl(qm.detach(), qs.detach(), pm, ps)
        train_post = gaussian_kl(qm, qs, pm.detach(), ps.detach())
        return a * torch.clamp(train_prior, min=fb) + (1 - a) * torch.clamp(
            train_post, min=fb
        )

    # --- evaluation rollouts ----------------------------------------------------

    @torch.no_grad()
    def warmup(self, batch: dict, steps: int) -> torch.Tensor:
        """Teacher-forced posterior filtering for `steps`; returns h at t=steps."""
        d = batch["demand"]
        dow_e = self.dow_emb(batch["dow"])
        h = d.new_zeros(d.shape[0], self.cfg.h_dim)
        for t in range(steps):
            pn = batch["price_norm"][:, t : t + 1]
            on = batch["order_norm"][:, t : t + 1]
            de = dow_e[:, t]
            e = self._encode(d[:, t : t + 1], pn, de)
            qm, qs = self.posterior(h, e)
            z = qm + qs * torch.randn_like(qs)
            h = self._recur(h, z, pn, on, de)
        return h

    @torch.no_grad()
    def open_loop(
        self, h: torch.Tensor, batch: dict, start: int, n_samples: int
    ) -> dict:
        """Prior rollout from t=start with the batch's real actions; no demand peeking.

        Each of the K samples evolves its own (h, z) chain. Returns per-step demand
        means and samples of shape (B, K, T-start).
        """
        B = h.shape[0]
        K = n_samples
        T = batch["demand"].shape[1]
        h = h.repeat_interleave(K, dim=0)
        dow_e = self.dow_emb(batch["dow"])
        means, samples = [], []
        for t in range(start, T):
            pn = batch["price_norm"][:, t : t + 1].repeat_interleave(K, dim=0)
            on = batch["order_norm"][:, t : t + 1].repeat_interleave(K, dim=0)
            de = dow_e[:, t].repeat_interleave(K, dim=0)
            pm, ps = self.prior(h)
            z = pm + ps * torch.randn_like(ps)
            dist = self.demand_dist(h, z, pn, de)
            means.append(dist.mean)
            samples.append(dist.sample())
            h = self._recur(h, z, pn, on, de)
        mean = torch.stack(means, dim=-1).view(B, K, T - start)
        sample = torch.stack(samples, dim=-1).view(B, K, T - start)
        return {"mean": mean, "sample": sample}
