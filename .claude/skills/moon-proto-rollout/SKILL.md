---
name: moon-proto-rollout
description: >-
  Roll out the proto + moon + lefthook + Renovate polyglot-monorepo pattern onto
  another repository. proto pins one version of every tool (.prototools), moon is
  the affected-aware task runner (.moon/ + a moon.yml per project), lefthook wires
  the git hooks, and Renovate automates dependency updates (mirroring the uv
  supply-chain cooldown). Use when asked to "set up moon", "add proto", "add
  renovate", "set up dependency automation", "roll out the moon/proto stack",
  "standardise tooling on this repo", or to reproduce this monorepo's
  build/lint/test/audit/dep-update setup elsewhere. Covers Rust (Cargo
  workspaces), Python (uv workspaces), and TypeScript projects.
---

# Roll out the moon + proto + lefthook stack

This skill reproduces a battle-tested polyglot-monorepo toolchain on a **target
repo**. The pattern, in one breath:

- **proto** owns *tool versions* — one pinned set for everyone, in `.prototools`
  at the repo root. Built-in tools (rust/python/node/bun/uv) need no plugin;
  long-tail tools (ruff, ty, gitleaks, sqruff, cargo-deny, lefthook) get a
  **vendored plugin manifest** under `proto-plugins/` that points at the tool's
  *official* upstream releases.
- **moon** owns *the build graph* — it knows every project and their deps, so it
  runs lint/test/build **only for what a change affects** and caches the rest.
  Config: `.moon/workspace.yml` + language-scoped task files + one tiny
  `moon.yml` per project.
- **lefthook** owns *git hooks* — pre-commit (format/lint/secret-scan) and
  pre-push (moon affected lint+test), all driven through proto's shims.
- **Renovate** owns *dependency updates* — one `renovate.json5` at the root keeps
  every ecosystem (cargo/uv/npm/proto/actions) current, mirrors the uv
  supply-chain cooldown via `minimumReleaseAge`, and auto-merges the low-risk
  majority once moon's affected CI gate is green.

Templates referenced below live in `templates/` next to this file. The hard part
— the vendored proto plugins — are ready to copy **verbatim**.

## Before you start: read the references

- `references/architecture.md` — the *why* and the non-obvious gotchas. **Read
  this first.** It will save you from the traps (Moon 2.x `inheritedBy` scoping,
  `toolchain: 'system'`, proto shims in non-interactive hooks, `cache: false`
  for audits).
- `references/proto-plugins.md` — how a vendored plugin works and how to author a
  new one when the target needs a tool not already templated.
- `references/renovate.md` — why Renovate completes the stack, what makes
  auto-merge safe, the proto `customManager` gotchas, and how to enable it.

## Procedure

Work top-down. Confirm scope with the user where marked ⚠.

### 1. Survey the target repo

Determine, by inspection (don't assume):

1. **Languages present** — Rust? Python? TypeScript? Something else?
2. **Project layout** — where do the buildable units live? For each, note its
   path, language, and whether it's a `library` or `application` (the moon
   `layer`).
3. **Existing dependency structure** — Cargo workspace? uv workspace? npm
   workspaces? moon *infers* Rust inter-crate deps from `Cargo.toml`; for other
   languages you set `dependsOn` by hand.
4. **What tools are already in use** — formatters, linters, type checkers,
   security scanners, and any existing version manager (mise, asdf, rtx,
   volta, rustup overrides). ⚠ These are what proto will replace — surface them
   before deleting anything.

### 2. Pin tools with proto (`.prototools`)

Copy `templates/.prototools` to the repo root and trim it to the languages and
tools the target actually uses. Rules:

- Always pin `proto` and `moon` themselves.
- Built-in tools (`rust`, `python`, `node`, `bun`, `uv`) get a top-level version
  line and **no** `[plugins]` entry.
- Every non-built-in tool needs a `[plugins]` entry with a `file://` locator AND
  a corresponding manifest under `proto-plugins/` (next step).
- Keep the `UV_EXCLUDE_NEWER` supply-chain cooldown only if the target uses uv.

Then copy only the needed plugin manifests from `templates/proto-plugins/` into
the target's `proto-plugins/`. They are official-release pointers and need no
edits. If the target needs a tool not templated here, author a new manifest per
`references/proto-plugins.md`.

Verify: `proto install` (or `proto use`) resolves and installs everything.

### 3. Set up moon

1. **`.moon/workspace.yml`** — copy `templates/.moon/workspace.yml`, then list
   every project as `id: 'relative/path'`. IDs must be unique; the convention
   for nested paths is to replace `/` with `-` (e.g. `io/database` →
   `database`, `rose/quartz/frontend` → `quartz-frontend`). Set
   `vcs.defaultBranch` to the target's real default branch.
2. **Language task files** — copy the relevant files from
   `templates/.moon/tasks/` (`rust.yml`, `python.yml`, `typescript.yml`). Each is
   scoped by `inheritedBy.language` — **this scoping is mandatory** (see
   architecture.md; without it a task file applies to *all* projects regardless
   of filename). Adjust commands to match the target's tools.
3. **Per-project `moon.yml`** — drop a minimal file in each project dir. Use
   `templates/project-templates/` as the starting point: set `layer`, `language`,
   and `dependsOn` for non-Rust internal deps.

Verify: `moon query projects` lists every project; `moon run :lint --affected`
runs the right subset.

### 4. Wire git hooks (lefthook)

Copy `templates/lefthook.yml`, trim jobs to the languages present. The critical
invariant: **every hook command prepends `PATH="$HOME/.proto/shims:$PATH"`** —
hooks run non-interactively and don't source the shell rc, so the repo must be
self-sufficient. Run `lefthook install` to register the hooks.

### 5. Supporting config

- **cargo-deny** (Rust): copy a `deny.toml` to the root; the `audit` task points
  at it. See the repo's `deny.toml` for the license allow-list shape.
- **`.gitignore`**: add `.moon/cache` and `.moon/docker` (regenerated, never
  committed). See `templates/gitignore.snippet`.
- **README**: add a "Setup" section telling devs to install proto, run
  `proto install`, `lefthook install`, and to put proto's shims on PATH in
  `~/.zshenv` (not just `~/.zshrc`). See `templates/readme-setup.md`.

### 6. Dependency automation (Renovate)

Copy `templates/renovate.json5` to the repo root. ⚠ Before doing so, confirm the
deploying lockfiles (`Cargo.lock`, `uv.lock`, `bun.lock`, `package-lock.json`) are
**committed** — Renovate's range/transitive updates and auto-merge are only safe
against a committed lock. Then:

- Set `minimumReleaseAge` to the **same window** as the target's `UV_EXCLUDE_NEWER`
  (this is what flips the passive uv cooldown into active automation).
- Trim the `matchManagers` group rules to the ecosystems actually present.
- Keep the `customManager` (it tracks `proto`/`moon`); add other concretely-pinned
  tools only if they have a known datasource.
- If the repo has archived or scaffolding-template paths, enable the
  `matchFileNames … enabled: false` rule so Renovate doesn't open PRs on them.

Validate the config, then enable it (the config is **inert** until the GitHub App
is installed):

```bash
npx --package renovate -- renovate-config-validator renovate.json5
```

`references/renovate.md` covers the *why*, the interlock that makes auto-merge
safe, the `customManager` gotchas, and the full enablement checklist (App install,
onboarding PR, required `affected` check).

### 7. Validate the rollout

Run, and confirm each succeeds:

```bash
proto install                       # all pinned tools resolve
moon --version                      # matches .prototools
moon query projects                 # every project listed
moon run :lint --affected           # affected-aware lint, all languages
moon run :test --affected           # affected-aware test
moon run :audit --query "language=rust"   # if Rust present
```

Then make a trivial change in one project and confirm `--affected` scopes the
run to just that project (and its dependents).

## Adapting, not transplanting

The target repo will differ. Do **not** blindly copy the project list, the rose/
blank/cocoon ids, or tools the target doesn't use. The *structure* transfers; the
*contents* must be derived from the target. When the target has a language or
tool this skill doesn't template, follow the same principles: pin it in proto
(vendoring a plugin if non-built-in), add a language task file scoped by
`inheritedBy`, a hook job that runs through proto shims, and — if it's a new
ecosystem — a Renovate manager + group rule so its deps stay current too.
