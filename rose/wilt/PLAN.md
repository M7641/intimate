# wilt — prototype plan

Model-based RL for joint **replenishment** (order quantity) and **markdown pricing** of
perishable retail goods. Dreamer-style world model (RSSM) on tabular data: learn a latent
demand model, train the policy by imagination against it.

Built to the agreed architecture: **learn only demand; hard-code inventory accounting,
the profit identity, and termination.** No network ever learns bookkeeping.

---

## 1. Framing

Finite-horizon stochastic control. Each period the agent picks `(order_qty, price)` and
maximises cumulative profit over a horizon ≥ shelf life + lead time.

- **Physical state** age-cohort on-hand vector `q[1..m]`, pipeline `o[1..L]`, calendar context.
- **Latent state (learned):** `h_t` (GRU history), `z_t` (stochastic demand-regime latent), `s_t = [h_t, z_t]`.
- **Reward (analytic, never learned):**
  `r_t = price·sales − holding·on_hand − waste_cost·spoiled − order_cost − stockout_penalty·unmet`,
  plus terminal salvage on remaining stock.
- **Transition (factored):** learned demand `d ~ p(d | price, z, ctx)` (negative binomial);
  known `sales = min(d, Σq)`; known FIFO/ageing/spoilage/pipeline accounting; learned
  latent transition `h' = GRU(h, z, a, ctx)` with prior `p(z|h)` and posterior `q(z|h, e)`.

## 2. Milestones & acceptance criteria

### M0 — Synthetic environment (Phase 0) — BUILT

Ground-truth demand: weekly seasonality × constant-elasticity price effect × AR(1)
log-demand regime, NegBin observation noise. Gym-style `step()`. Exact FIFO perishable
accounting shared as pure functions (numpy scalar + torch batched).

Acceptance:

- Unit tests pass: mass conservation (initial + arrived = sold + spoiled + remaining),
  FIFO oldest-first issue, exact lead-time arrival, seed reproducibility,
  numpy/torch accounting parity (flows and reward).
- Heuristic policy episode produces positive profit, plausible waste % and fill rate.

### M1 — World model + rollout fidelity (Phase 1) — BUILT

Encoder (calendar embed + MLP over log demand & price) → RSSM (GRUCell, Gaussian
prior/posterior, KL balancing + free bits) → NegBin demand head conditioned on
`(h, z, price, ctx)`. No observation reconstruction — the only generative head is demand
(JEPA-flavoured: predict in likelihood space).

Trained on trajectories collected by a randomised base-stock + ε-random-price behaviour
policy (price variation is what identifies elasticity).

Acceptance (fidelity protocol, see §5):

- One-step posterior NLL on held-out episodes is finite and improves over training.
- Open-loop predicted demand mean beats the seasonal-naive baseline (dow-mean) against
  the **oracle mean** (the simulator's true λ_t, known by construction) for k ≤ 7.
- 80 % central interval covers realised demand in roughly [0.70, 0.90].
- Imagined inventory/profit roll (sampled demand through the _same_ analytic accounting)
  tracks true cumulative profit within ~15 % relative error over the rollout window.

Measured (seed 0, 224 train / 36 val episodes, 40 epochs, W=14, K=64):

- val posterior NLL 16.4 → 3.45 (plateaued); KL ≈ 2 nats (posterior in use, no collapse).
- open-loop sMAPE vs oracle mean: 0.50 (k=1), 0.41 (k≤7), 0.40 (all) vs seasonal-naive
  0.46 / 0.44 / 0.44 — beats naive for k≤7 and overall; k=1 slightly worse (see risks).
- 80 % interval coverage of realised demand: 0.808.
- imagined accounting relative error: sales 6.6 %, profit 17.4 % (just above target),
  waste 4.7 (true waste is near zero, so the relative metric is denominator-dominated —
  switch to absolute units/period before relying on it).

### M2 — Imagination actor-critic (Phase 2) — NOT BUILT YET

Actor `π(order_idx, price_idx | s, q, o)` with constraint masking (MOQ/case-pack via the
discrete order grid; optional monotone non-increasing price mask for the seasonal/
single-batch regime); critic `v(s, q, o)`; imagined rollouts of length H ≥ m + L using
prior demand samples + the torch accounting + analytic reward; λ-returns; entropy bonus.
Guardrails: demand-head ensemble (K heads, take pessimistic quantile of profit), penalty
for prices outside the historical support, short horizon, posterior re-grounding.

Acceptance: dreamed policy ≥ myopic-greedy baseline on ≥ 100 fresh true-env episodes;
zero constraint violations.

### M3 — Evaluation harness (Phase 3) — NOT BUILT YET

Baselines: (a) myopic greedy (1-step expected profit under the learned demand model),
(b) classic base-stock replenishment + rule-based markdown (markdown when projected
spoilage exceeds a threshold). Metrics: cumulative profit, waste %, fill rate /
service level, world-model accuracy (the M1 metrics, re-reported on the eval seeds).

Acceptance: a single `run_eval.py` table comparing dreamed vs both baselines over common
random seeds; dreamed wins or the loss is explained by a measured model defect.

### M4 — Real-data notes (Phase 4) — NOTES ONLY

- **Censored likelihood:** observed sales are `min(d, stock)`; when stock ran out the
  demand likelihood term becomes the NegBin survival function `P(d ≥ sales)`. The NLL in
  `world_model/model.py` is the single place to change.
- **Price support:** estimate the historical price density per context; penalise (or mask)
  actions outside it. Hooks live in the actor's mask and the imagination reward.
- **Real calendars:** swap the synthetic dow features for real calendar/promo features in
  the encoder — the encoder is the single seam.

## 3. Component breakdown & file layout

Layout mirrors the learned-vs-hardcoded split:

```
wilt/
├── pyproject.toml
├── PLAN.md / README.md
├── scripts/
│   ├── run_phase0.py        # env smoke run + invariants
│   └── run_phase1.py        # collect → train → fidelity report
└── wilt/
    ├── config.py            # EnvConfig / ModelConfig / TrainConfig dataclasses
    ├── env/                 # EXACT, hard-coded — owns the ground truth
    │   ├── demand.py        # GroundTruthDemand (the thing the model must learn)
    │   ├── accounting.py    # pure FIFO/ageing/pipeline step + reward (numpy & torch)
    │   ├── simulator.py     # PerishableRetailEnv (gym-style)
    │   └── test_*.py        # co-located tests
    ├── data/
    │   ├── collect.py       # behaviour policy + episode collection
    │   └── buffer.py        # episodes → batched torch tensors
    ├── world_model/         # LEARNED
    │   ├── model.py         # encoder + RSSM + NegBin head; observe / warmup / open_loop
    │   ├── train.py         # training loop
    │   └── test_model.py
    ├── policy/              # Phase 2 (stub): actor-critic + action masks
    ├── baselines/           # Phase 3 (stub): myopic greedy, base-stock + markdown rules
    └── eval/
        └── fidelity.py      # Phase 1 acceptance: open-loop rollout fidelity report
```

## 4. Key interfaces

```python
# env — gym style
obs, info = env.reset(seed)                       # obs: on_hand, pipeline, dow, t, last_demand
obs, r, terminated, truncated, info = env.step((order_idx, price_idx))

# accounting — pure, shared verbatim between env and imagination
q, o, flows = step_inventory_np(q, o, order_qty, demand)     # flows: sales/spoiled/unmet/arrived/end_on_hand
q, o, flows = step_inventory_torch(q, o, order_qty, demand)  # batched; same semantics (tested for parity)
r = reward_torch(cfg, price, order_qty, flows)

# world model
loss, metrics = model.observe(batch)              # posterior filtering: NLL + balanced KL
h = model.warmup(batch, warmup)                   # teacher-forced posterior up to t=warmup
out = model.open_loop(h, batch, start, n_samples) # prior rollout: demand means + samples (B,K,H)

# policy (Phase 2)
logits_order, logits_price = actor(s, q_norm, o_norm)   # masked before softmax
v = critic(s, q_norm, o_norm)
```

## 5. Evaluation design (Phase 1 fidelity)

Synthetic-env superpower: the **oracle demand mean λ_t is known**, so model error can be
separated from irreducible NegBin noise.

Protocol, on held-out episodes:

1. Warm up the posterior for W=14 steps (teacher forcing with true demand/actions).
2. Open-loop the remaining T−W steps: prior z, true actions, no peeking at demand.
3. Report:
   - sMAPE of predicted mean vs **oracle mean** at k=1, k≤7, k≤14, all — vs the
     seasonal-naive (dow-mean of training demand) baseline.
   - 80 % interval coverage of realised demand.
   - Inventory fidelity: push K demand samples through the torch accounting from the true
     `(q, o)` at t=W with the true actions; compare cumulative profit / waste / sales
     against the realised episode.

## 6. Risks & assumptions

| Risk                                                             | Mitigation                                                                 |
| ---------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Elasticity unidentifiable if behaviour policy never varies price | collection policy picks a random ladder price 50 % of the time             |
| Posterior collapse (KL → 0, demand head ignores z)               | free bits + KL balancing; watch KL in logs                                 |
| Compounding open-loop error                                      | short imagination horizon (H ≈ m + L); fidelity gate before Phase 2        |
| Model exploitation by the policy                                 | Phase 2: ensemble pessimism, price-support penalty, posterior re-grounding |
| NegBin numerical instability                                     | parameterise via `logits = log μ − log r`; clamp `log μ`, floor `r`        |
| Sim-to-real gap (censoring, price support)                       | Phase 4; NLL and actor mask are the two designated seams                   |

Assumptions: demand fully observed (synthetic); single SKU/store; daily periods; discrete
action grid; Gaussian latent (revisit discrete latent if KL behaves badly); continuous
replenishment regime (monotone markdown mask reserved for a seasonal mode).

## 7. Later Rust port (note only)

Serving the trained policy is a pure-inference problem: export actor (and encoder/GRU
step) via TorchScript/ONNX; port `accounting.py` 1:1 to Rust (it is deliberately pure and
tested for parity, so a Rust twin can be golden-tested against the Python flows). Fits the
existing cocoon/ouroboros deploy path. Training stays in Python.
