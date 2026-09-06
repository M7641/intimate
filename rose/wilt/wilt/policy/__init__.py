"""Phase 2 (not built yet): imagination actor-critic.

Planned contents (see PLAN.md §M2):
  actor.py   - pi(order_idx, price_idx | s, q_norm, o_norm) with constraint masking
               (case-pack grid; optional monotone non-increasing price for the
               seasonal/single-batch regime)
  critic.py  - v(s, q_norm, o_norm)
  dream.py   - imagined rollouts: prior demand samples + env/accounting.py (torch)
               + analytic reward; lambda-returns; entropy regularisation
  Guardrails: demand-head ensemble pessimism, price-support penalty, short horizon.
"""
