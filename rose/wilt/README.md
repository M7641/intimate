# wilt

Model-based RL prototype for joint **replenishment** and **markdown pricing** of
perishable retail goods. Dreamer-style RSSM world model on tabular data; the policy
will be trained by imagination against the learned demand model (Phase 2).

Design principle: **learn only demand; hard-code inventory accounting, the profit
identity, and termination.** See [PLAN.md](PLAN.md) for milestones and architecture.

## Quickstart

```sh
uv sync
uv run pytest                          # env invariants, accounting parity, model shapes
uv run python scripts/run_phase0.py    # env smoke run
uv run python scripts/run_phase1.py    # collect -> train -> fidelity report (~2 min CPU)
```

## Status

- Phase 0 (synthetic env, exact accounting): done
- Phase 1 (world model + rollout fidelity): done
- Phase 2 (imagination actor-critic): stub in `wilt/policy/`
- Phase 3 (baselines + eval harness): stub in `wilt/baselines/`
- Phase 4 (real data: censored likelihood, price support): notes in PLAN.md
