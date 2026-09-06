
CI/CD is a **conveyor of trust**: a change moves from a developer's keystroke to production through a series of gates, each cheaper to pass than the damage it prevents, and the machine — not a human's memory — enforces every gate.

This pipeline is a series of gates that a change must pass through to reach the end, where it becomes a fully released change. 

Each gate attempts to catch issues early and cheaply. Each gate becomes more expensive, but covers more and more to further increase trust that change is not going to cause any damage.

### Link 0 - A repeatable toolchain

The code runs on the exact same versions everywhere. A tool like proto ensures that everyone running the code has the same tools.

The next step would be to have a repeatable OS, such as Nix. 

### Link 1 - Local fast feedback

This uses something like pre-commit hooks to automatically lint and do cheap fixes on staged changes.

This step has to be cheap as we don't want to obstruct the developers' flow. 

### Link 2 - Commit & change hygiene - Weakness

Why have changes been made? The commit associated with the change should explain why that line was added or edited.

**Conventional Commits** (`type(scope): subject`)

Is the format we should aim to use. This gives us a change log and an automated version.

This link is harder than the ones that came before it, as you don't get the reward immediately; rather, it's something you gain at the end of the process, when you have a good history and a good change log that you can use to communicate with others clearly.

### Link 3 - Local pre-integration gate (`pre-push`)

**The idea.** Before code leaves your machine for shared history, run the
*expensive* checks — the real test suites and cross-project lint — but run them
**only for what your change affects**, so the cost stays bearable. This is the
last gate the developer sees before the server takes over; its job is to make
the server gate *usually pass on the first try*.

We use the pre-push for this; it takes a minute or two, and we should make sure our tests are quick when possible.

### Link 4 — Server-side CI (the authoritative gate)

**The idea.** Everything above lives on the developer's machine and is,
ultimately, _optional_ — hooks can be skipped. The **authoritative** gate is the
one that runs **on a neutral machine you control, that no contributor can
bypass, and whose verdict decides whether a change may merge.** This is the line
between "we have good habits" and "we have a system."

**World-class.**

- A CI workflow is triggered **on every pull request** (and on `main`).
- It runs on a **clean runner**, installs the _pinned_ toolchain (Link 0), and
  invokes **the same commands** developers run — no bespoke CI script.
- It is **affected-aware and cached** so PR feedback is minutes, not tens of
  minutes: the runner restores the `moon` cache and only re-runs what changed
  versus the PR base.
- Its checks are wired as **required status checks** on the protected branch
  (Link 5), so a red pipeline _physically prevents_ merge.
- It uses **least-privilege, pinned actions** (third-party actions pinned to a
  commit SHA, not a moving tag) and short-lived cloud credentials (OIDC), never
  long-lived secrets.

Tool families: **GitHub Actions** / GitLab CI / CircleCI / Buildkite. The
pattern is identical across all of them.


### Link 5 — Branch protection, review & ownership

**The idea.** A gate is only a gate if it cannot be walked around. Branch
protection is the enforcement layer that says: _no change reaches `main` except
through a pull request that (a) passed the required checks and (b) was reviewed
by someone who owns that code._ It converts the CI verdict from information into
a **precondition**.


### Link 6 — Build the artifact **once**

**The idea.** The thing you test and the thing you ship must be **byte-for-byte
the same object.** If you build a container for staging and then _rebuild_ it
for production, you have shipped something no one tested — the base image moved,
a dependency floated, the build was not reproducible. World-class pipelines
build **one immutable artifact**, stamp it with the commit SHA, and then
_promote that same artifact_ through every environment.

This is a deep problem we will not be able to get around, given we can't provide built images to the platform.

If we could, our GitHub workflow could run our vulnerability-scanning tools and the like. Then we would have a totally tested artefact in production. 

### Link 7 — Continuous Deployment

**The idea.** CI answers _"is this change good?"_ CD answers _"get the good
change to users, safely, repeatably, and without a human copy-pasting
commands with production credentials."_ The deploy must be a **script the
machine runs**, identical every time, triggered by an event (a merge, a tag),
and logged — so "who deployed what when" has an answer, and so anyone can
trigger a deploy by merging, not by knowing the incantation.

**World-class.** On merge to `main` (or on a version tag), a pipeline job
deploys the **already-built artifact** (Link 6) to an environment.

So we do have a repeatable script to deploy, but it's not automatic. Our secrets process does make this difficult also.

### Link 8 — Environments & promotion

**The idea.** You do not want the first machine to run your change to be the one
customers use. A change should **earn its way to production** by first proving
itself in progressively more production-like environments. Each promotion is a
_decision gate_: automatic where you have confidence, manual (a click) where you
want a human in the loop.

**World-class.** A defined ladder — e.g. `dev → staging → prod` — where a merge
auto-deploys to staging, an automated smoke/e2e suite runs there, and promotion
to prod is either automatic (true continuous _deployment_) or a single approving
click (continuous _delivery_).

This is to say, we have a UAT environment or at least a test environment which is production but not in all the users hands. 

### Link 9 — Close the loop: verify, observe, roll back

**The idea.** Shipping is not the end of the change's life; it is the start of
its life _in production_. A world-class pipeline **verifies the deploy actually
worked** (health checks, smoke tests), **watches** the running system (metrics,
logs, alerts), and makes **undo cheap** — because the fastest incident response
is "roll back to the last known-good artifact," which is only possible _because_
Link 6 kept every artifact immutable and addressable.

### Link 10 (cross-cutting) — Keep dependencies fresh & the supply chain honest

**The idea.** Pinned dependencies (Link 0) solve reproducibility but create a
new risk: pins _rot_. Stale dependencies accumulate security holes silently. The
answer is not "float everything" (that breaks reproducibility) but
**automation that proposes small, reviewable, tested upgrades continuously** —
each one a PR that must pass the very CI gate you built in Link 4.

**World-class.** A bot opens dependency-bump PRs on a schedule; each PR runs the
full CI suite, so a bump that breaks a test is caught _as a red PR_, not in
production. A **cooldown** (don't adopt a release younger than N hours/days)
blunts supply-chain attacks that rely on a compromised package being pulled
immediately. Tool families: **Renovate** / Dependabot; the cooldown
(`minimumReleaseAge` / `UV_EXCLUDE_NEWER`).

## The principles behind the whole chain

If you internalise nothing else, internalise these — they generate every
decision above:

1. **Fail fast, fail cheap.** Push each class of error to the earliest, cheapest
   gate that can catch it. Feedback latency is the metric that matters.
2. **The same command everywhere.** Developers and CI invoke the _identical_
   task (`moon run :test --affected`). No drift, no "CI-only" scripts. The gate
   is re-invoked, never re-implemented.
3. **The machine is the enforcer.** A gate a human can forget is not a gate.
   Advisory locally, _mandatory_ server-side.
4. **Build once, promote many.** The tested artifact and the shipped artifact
   are the same immutable object, addressed by commit SHA.
5. **Everything is code, in the repo.** Toolchain pins, hooks, CI workflows,
   deploy definitions, environment config — all reviewable, all versioned, all
   diffable. Nothing lives only in someone's head or a UI.
6. **Only run what changed.** The affected graph + caching is what makes a
   thorough pipeline _fast enough to keep_. A slow pipeline gets bypassed, and a
   bypassed gate is no gate.
7. **Small changes, short-lived branches.** CI/CD rewards trunk-based
   development: tiny PRs merge often, each fully tested, so integration pain and
   rollback blast-radius both shrink.
8. **Close the loop.** Observe production and make undo cheap; a deploy you
   cannot verify or reverse is a gamble, not a release.

## What the developer does 

### Step 1 — Start clean: sync and branch

Never build on stale foundations. Begin every piece of work from an up-to-date
`main`, then branch:

```bash
git switch main && git pull        # start from the latest shared truth
git switch -c fix/annual-countsize # a short-lived branch, one job to do
```

_The practice — trunk-based development._ Branches are **short-lived** (hours to
a day or two, not weeks). A branch that lives for two weeks silently drifts from
`main`; merging it becomes a big-bang integration that no gate anticipated. Small
branches merge cleanly and keep the affected-set small, so CI stays fast.

### Step 2 — Keep the change small and single-purpose

One branch does _one_ thing. A fix _or_ a feature _or_ a refactor — not all
three. This is the highest-leverage habit a programmer has, because everything
downstream scales with diff size: small diffs are reviewed properly, tested
quickly (the affected graph touches fewer projects), and — if they break
production — rolled back without collateral damage.

> If you cannot describe the change in one Conventional Commit subject line,
> it is probably two changes.

### Step 3 — Run the gates locally, as you work

Do not treat the hooks as your test strategy — treat them as a _safety net_ for
when you forget. While you work, run the same command CI will run, yourself:

```bash
moon run :test --affected   # exactly what pre-push and CI run
moon run :lint --affected
```

_The practice — no surprises._ The goal is that when you push, the local
`pre-push` gate and the server CI gate both pass **on the first try**, because
you already saw green. Discovering a failure locally costs you two minutes;
discovering it in a red PR costs the whole team a context-switch.

### Step 4 — Commit in small, honest, conventional steps

Let `pre-commit` do the janitorial work (formatting, lint fixes, secret scan) —
that is _its_ job, not yours. Your job is the message and the granularity:

```bash
git add -p                            # stage one logical hunk at a time
git commit                            # cz check enforces type(scope): subject
```

Write [Conventional Commits](/general_notes/changelog_and_commits/) because Link
2 turns them into changelogs and versions automatically. **Do not bypass the
hooks.** `git commit --no-verify` and `LEFTHOOK=0` exist for genuine
emergencies (a broken hook blocking a hotfix), _not_ for "the linter is
annoying." Every bypass is a gate you personally disabled for everyone
downstream.

### Step 5 — Push, and let the pre-push gate run

```bash
git push -u origin HEAD
```

The `pre-push` hook fires `moon run :lint :test --affected` and boots Podman for
the container-backed suites. Let it finish. If it fails, **you fix it before the
push lands on the server** — that is the entire point of the gate sitting where
it does.

### Step 6 — Open a pull request that reviews itself

A PR is a request for _another human's judgement_, so make that judgement cheap
to give:

- **Explain the _why_,** not just the what — the diff shows the what.
- **Keep it small** (Step 2 pays off here again).
- **Link the work item** and fill the PR template.
- Let `CODEOWNERS` route it to the people who own the touched code.

_The practice — the author serves the reviewer._ A PR that needs a 20-minute
verbal explanation is a PR that is too big or under-described.

### Step 7 — Own your red CI

When CI runs on the PR, its verdict is **yours to clear** — not the reviewer's,
not "flaky, I'll re-run it." A red pipeline on your PR is your top priority,
because until it is green:

- your change cannot merge (required checks, Link 5), and
- you are blocking anyone who wants to branch from a clean `main`.

If CI fails and it passed locally, that gap is information — usually an
un-pinned dependency or an environment assumption — and worth understanding, not
just re-running until it goes green.

### Step 8 — Review others as you would be reviewed

You are a _link in someone else's chain_ too. Review promptly (a stale PR rots),
review the _design and correctness_ (let the machine handle style — it already
did), and approve only what you would be comfortable owning if it broke at 2am.

### Step 9 — Merge, and hand off to the pipeline

Once green and approved, merge. From here the machinery takes over: build-once
(Link 6), deploy (Link 7), promote (Link 8). Keep the shared history clean —
prefer a tidy, linear merge; never force-push a branch someone else is building
on.

### Step 10 — Stay responsible past the merge button

**Ownership does not end at merge.** This is the practice that separates a
professional from a code-thrower. After your change ships:

- **Watch it land** — check the deploy succeeded and the post-deploy signals
  (the Nimbus ops dashboard, the Slack alert watchers wired into the deploy
  specs) stayed quiet.
- **Be ready to roll back** — if something breaks, the correct first move is
  _revert to the last known-good artifact_, not a panicked hot-patch. Reverting
  is not failure; it is the system working.
- **A revert is cheap _because_ you kept the change small** — Step 2, collected.

### The cardinal rules, in one place

| Do | Don't |
|----|-------|
| Start from fresh `main`, short-lived branch | Let a branch live for weeks |
| One logical change per branch/PR | Mix fix + feature + refactor |
| Run `moon run :test --affected` before pushing | Rely on the hooks as your test plan |
| Let `pre-commit` fix hygiene | `--no-verify` / `LEFTHOOK=0` to dodge lint |
| Write Conventional Commits | Vague "wip" / "fix stuff" messages |
| Clear your own red CI first | Re-run until green, or leave it for review |
| Explain the _why_ in the PR | Ship a 40-file PR with no description |
| Watch your change reach prod | Merge and walk away |
| Roll back fast when in doubt | Hot-patch prod under pressure |

The through-line: **the machine enforces the rules so you can spend your
judgement on the things that matter** — small, honest, well-described changes
that you shepherd all the way to a healthy production. Do that, and the whole
chain above simply works around you.
