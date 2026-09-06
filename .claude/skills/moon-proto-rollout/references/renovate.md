# Dependency automation with Renovate

Renovate is the fourth pillar of this stack. proto/moon/lefthook keep the
toolchain pinned, built, and gated; Renovate is what keeps the *dependencies*
moving — safely — instead of rotting behind a passive cooldown.

## Why it belongs in this stack

The proto stack ships a supply-chain cooldown: `UV_EXCLUDE_NEWER` (and
`[tool.uv] exclude-newer`) refuse any dependency uploaded in the last N hours from
entering a lock. But that cooldown is **passive** — uv blocks fresh deps and
*nothing ever advances the lock*, so dependencies sit N hours behind *forever*. A
cooldown without automation isn't safety, it's rot with a delay.

Renovate has a native mirror of that window — **`minimumReleaseAge`**. Set it to
the *same* value as `UV_EXCLUDE_NEWER` and the cooldown flips from passive to
active: Renovate opens a PR the moment a release crosses the age threshold. Same
window, now automated.

## Why Renovate (not Dependabot)

A moon/proto repo is polyglot — typically Cargo, uv, bun/npm, proto, and GitHub
Actions in one tree. Dependabot supports neither uv nor proto well and groups
poorly across a monorepo. Renovate covers every ecosystem in one tool, with the
grouping + auto-merge a fan-out monorepo needs to avoid PR spam — and the
`minimumReleaseAge` cooldown mirror above.

## What it manages

| Ecosystem | Renovate manager | Files | Notes |
| --- | --- | --- | --- |
| Rust | `cargo` | `*/Cargo.toml`, `*/Cargo.lock` | Range bumps + lockfile maintenance. Workspace path-deps ignored automatically. |
| Python | `pep621` | `*/pyproject.toml`, `*/uv.lock` | uv-aware; updates constraints and the lock. |
| JS | `npm` | `*/package.json`, `bun.lock` | Reads text-format `bun.lock`; confirm bun support when enabling. |
| Toolchain | `customManagers` (regex) | `.prototools` | No native proto manager — a regex manager tracks the pinned tools' upstream releases. |
| CI | `github-actions` | `.github/workflows/*.yml` | Enabled by `config:recommended`; keeps action versions current. |

## The interlock that makes auto-merge safe

Three pieces have to be true together — this is *why* patch/dev-dep auto-merge is
safe rather than reckless:

1. **Committed lockfiles.** Renovate can bump transitive deps and CI tests a
   frozen tree. Auto-merge is only safe where the deploying lock is committed —
   so commit `Cargo.lock`, `uv.lock`, `bun.lock`, `package-lock.json` *before*
   enabling Renovate.
2. **The `moon --affected` gate.** Only the touched projects are re-linted/tested,
   so a dep PR validates fast and precisely. The `cargo-deny` audit already wired
   into CI is the supply-chain safety net.
3. **`minimumReleaseAge`.** The cooldown is honoured *before* the PR exists.

```
upstream release ──(wait the cooldown, same as uv)──▶ Renovate opens a grouped PR
        │
   moon ci runs on AFFECTED projects only (lint + test + audit)
        │
        ├─ green + patch / devDep / lockMaintenance ──▶ auto-merge, zero touch
        └─ green + minor / major ────────────────────▶ waits for review
```

Net effect: dependencies stay ~one-cooldown fresh instead of one-cooldown behind
forever, with no human toil for the low-risk majority.

## The proto customManager — the non-obvious part

proto has no native Renovate manager, so a **regex customManager** tracks tool
versions in `.prototools`. Two gotchas:

- **Only concretely-pinned tools are trackable.** This stack pins `proto` and
  `moon` to real versions; most other tools are pinned to `latest`/`stable`, which
  Renovate cannot bump (there's no version to compare). The template regex matches
  `(?<currentValue>\d…)` — starting with a digit — so it captures `proto`/`moon`
  and silently skips the `latest`/`stable` lines. If the target pins other tools
  to concrete versions with a known datasource, add them to the manager.
- **`fileMatch` vs `managerFilePatterns`.** The field was renamed to
  `managerFilePatterns` in newer Renovate; older versions only know `fileMatch`.
  The template uses **`fileMatch`** because it works on both (newer Renovate
  auto-migrates it with a harmless notice). Validate with
  `npx --package renovate -- renovate-config-validator renovate.json5`; if your
  validator is newer than the deployed App, expect a "config migration" notice on
  `fileMatch` — that's fine.

Both `proto` and `moon` live in separate `moonrepo/*` GitHub repos with
`v`-prefixed release tags, so `datasource: github-releases` + default versioning
resolves them without an `extractVersion`.

## What Renovate does NOT do (honest limits)

- **Incoming deps only, not outgoing releases.** Renovate won't publish your own
  libraries or fix versionless `git+…@main` consumption — that's *release
  automation* (tags + `release-please`/`cargo-release`, then pinning consumers),
  a separate piece of work.
- **Config alone is inert.** `renovate.json5` does nothing until the **Renovate
  GitHub App** is installed on the repo, or a self-hosted
  `renovatebot/github-action` runs on a schedule.

## Enablement checklist

1. Confirm the deploying lockfiles are committed (the prerequisite above).
2. Copy `templates/renovate.json5` to the repo root; trim the `matchManagers`
   groups to the ecosystems present and set `minimumReleaseAge` to match the
   target's `UV_EXCLUDE_NEWER`. Add the archived/template `matchFileNames: …,
   enabled: false` rule if the repo has such paths.
3. Validate: `npx --package renovate -- renovate-config-validator renovate.json5`.
4. Install the **Renovate GitHub App** on the repo (or add a self-hosted action).
5. Merge Renovate's onboarding PR, reconciling its proposed diff with the config.
6. Confirm auto-merge requires the **affected** CI check (the same one branch
   protection requires).
