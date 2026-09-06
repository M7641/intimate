---
name: impeccable-stack
description: >-
  Runs the full impeccable improvement loop on a targeted code set with minimal
  supervision: audit → action the audit → critique → action the critique →
  polish, then one verification re-run. A thin orchestration layer over the
  installed `impeccable` skill — it chains `$impeccable audit`, the recommended
  fix commands, `$impeccable critique`, and `$impeccable polish` automatically,
  suppressing each command's "ask the user / run these one at a time" stop so the
  agent self-drives. Use when the user says "run the impeccable stack", "impeccable
  this", "autopilot the UI improvements", "audit and fix this component", "do the
  full impeccable pass on <target>", or wants frontend code improved end-to-end
  without babysitting each step. For a single isolated step (just an audit, just a
  critique, just polish), use `impeccable` directly instead.
---

# Impeccable stack — autonomous audit → fix → critique → fix → polish

A minimal driver that runs the installed **`impeccable`** skill's commands
back-to-back on one **target** (a file, directory, component, or route),
actioning each report as it lands. The whole point is **little supervision**: the
underlying `audit` and `critique` commands are written to stop and ask the user
what to fix; this skill overrides that and just does the work.

> Requires the `impeccable` skill to be installed globally at
> `$HOME/.agents/skills/impeccable/` (it is). This skill never reimplements
> impeccable's checks; it only sequences them and auto-applies their
> recommendations.

## Operating contract (read first)

- **Autonomy: quasi-total.** Execute every recommended command (P0 → P1 → P2) without
  pausing. Pick sensible defaults for any "Ask the User" step. **Stop and ask only**
  when a fix would require guessing a *design-system principle* — `polish.md` forbids
  guessing those, and so do we. P3 polish-only items: fold into the final `polish`
  pass, don't spawn separate commands for them.
- **Checkpoint between phases.** Before phase 1, and after each phase's fixes, create a
  git checkpoint so the user can review or revert any phase independently:
  ```bash
  git add -A && git commit -m "impeccable-stack: <phase> on <target>" --no-verify
  ```
  If the working tree is dirty at start, commit or stash first and say so — never mix
  the user's in-flight edits into a phase checkpoint.
- **One verification re-run.** After polish, re-run `audit` and `critique` once, report
  before/after scores, then stop. Do not loop further.
- **Stay in scope.** Only touch the target the user named. The audit/critique may notice
  neighbouring issues; note them, don't wander into them.

## Setup (once, up front)

Run impeccable's own setup before any phase — skipping it produces generic output:

1. **Load context** (PRODUCT.md / DESIGN.md), anchoring the path on `$HOME` since the
   skill lives in the home dir, not the repo:
   ```bash
   node "$HOME/.agents/skills/impeccable/scripts/load-context.mjs"
   ```
   Consume the full JSON. If `PRODUCT.md` is missing/placeholder, run `$impeccable teach`
   first (this is the one acceptable up-front interruption), then continue.
2. **Determine register** (brand vs product) for the target, per impeccable's setup rules.
   Cache it for the whole run.
3. **Snapshot the baseline.** Record the starting state and make the first git checkpoint.

## The loop

### Phase 1 — Audit, then action it

1. Run `$impeccable audit <target>`. Let it produce its full report: the **Audit Health
   Score (??/20)**, the P0–P3 findings, and the **Recommended Actions** list of
   `$impeccable <command>` calls.
2. **Ignore the trailing "you can ask me to run these one at a time" prompt.** Instead,
   execute the recommended commands yourself, in priority order (all P0, then P1, then P2).
   Each recommended command carries audit context — pass that context through so e.g.
   `$impeccable optimize` knows which expensive animation to fix.
3. Apply the fixes as real code edits to the target.
4. Git checkpoint: `impeccable-stack: audit fixes on <target>`.

### Phase 2 — Critique, then action it

1. Run `$impeccable critique <target>`. It launches its two isolated assessments
   (LLM design review + deterministic detector) and produces the **Design Health Score
   (??/40)**, priority issues, and persona red flags.
2. **The critique reference ends with an "Ask the User" step (priority / intent / scope /
   constraints). Do not ask — answer it from the run's own defaults:**
   - *Priority direction* → follow the report's own P0→P1→P2 ordering.
   - *Design intent* → **preserve the existing tone**; treat tonal "mismatches" as intentional
     unless PRODUCT.md/DESIGN.md explicitly contradicts the current design. Never restyle the
     brand on a guess.
   - *Scope* → all P0 and P1 issues, plus P2 issues whose fix is a single recommended command.
   - *Constraints* → respect anything the user marked off-limits when invoking the stack; otherwise none.
3. Execute the resulting Recommended Actions in priority order, applying fixes as real edits.
4. Git checkpoint: `impeccable-stack: critique fixes on <target>`.

### Phase 3 — Polish

1. Run `$impeccable polish <target>` as the final pass (both `audit` and `critique` end by
   recommending `polish` as the last step — this is that step, run once for the whole stack).
2. **Design-system ambiguity is the one hard stop.** If polish can't resolve a design-system
   principle without guessing, pause and ask the user that single question, then resume.
3. Git checkpoint: `impeccable-stack: polish on <target>`.

### Phase 4 — Verify once

1. Re-run `$impeccable audit <target>` and `$impeccable critique <target>` a single time.
2. Report a short before/after table — Audit Health (x/20 → y/20), Design Health (x/40 → y/40),
   and the count of P0/P1 issues resolved.
3. **Stop.** Do not start another fix round even if new low-severity items appear; list any
   residual P2/P3 items as "left for a follow-up" and end.

## Final report to the user

Keep it tight:

- **Before → after scores** (audit /20, critique /40).
- **What was fixed**, grouped by phase, each line naming the `$impeccable` command that did it.
- **Checkpoints created** (the git commits), so the user can review or revert any phase.
- **Left for follow-up**: any residual P2/P3 items you deliberately didn't action.

## Guardrails

- **Never** silently restyle brand identity, change copy tone, or rework a flow on a guess —
  that's the design-system hard stop.
- **Never** loop past the single verification re-run — bounded by design.
- **Never** skip impeccable's setup (context + register); generic output is the failure mode.
- **Never** action issues outside the named target.
- If a recommended command would conflict with a previous phase's fix (e.g. `bolder` then
  `quieter` on the same element), the **later** phase's intent wins — the stack converges, it
  doesn't oscillate.
