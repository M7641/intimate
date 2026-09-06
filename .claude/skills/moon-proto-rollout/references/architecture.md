# Architecture & gotchas

The *why* behind the pattern, and the non-obvious traps. Read this before
editing any config.

## The three tools, and the boundary between them

| Tool        | Owns                     | Config                                  |
| ----------- | ------------------------ | --------------------------------------- |
| **proto**   | tool *versions*          | `.prototools` (+ `proto-plugins/`)      |
| **moon**    | the *build/task graph*   | `.moon/` + a `moon.yml` per project     |
| **lefthook**| *git hooks*              | `lefthook.yml`                          |

Keep the boundary clean: proto never runs tasks, moon never pins versions.
moon tasks use `toolchain: 'system'` so they invoke whatever proto put on PATH.

## proto: one pinned toolchain for everyone

`.prototools` is the single source of truth for tool versions. Two kinds of tool:

- **Built-in** (`rust`, `python`, `node`, `bun`, `uv`): proto ships the plugin;
  just a top-level version line.
- **Everything else** (`ruff`, `ty`, `gitleaks`, `lefthook`, `sqruff`,
  `cargo-deny`): NOT built-in. Each gets a `[plugins]` entry with a `file://`
  locator pointing at a **vendored** manifest in `proto-plugins/`.

### Why vendored plugins (and not community ones)

A community plugin fetched at runtime is unreviewed third-party code in your
supply chain, and can break or disappear. The vendored manifests in
`proto-plugins/` are thin, reviewed pointers to each tool's **official upstream
GitHub releases**. They live in-repo, so the toolchain is reproducible and
auditable. See `proto-plugins.md` for how to author one.

### `UV_EXCLUDE_NEWER`

The `[env]` cooldown (`UV_EXCLUDE_NEWER = "48 hours"`) is a supply-chain guard:
no dependency uploaded in the last 48h can enter a lock. Only relevant with uv.

## moon: scoping is via `inheritedBy`, NOT the filename

**The single biggest trap.** In Moon 2.x, a file under `.moon/tasks/` does NOT
auto-scope by its name. `.moon/tasks/rust.yml` does not magically apply only to
Rust projects — *the filename is decorative*. Scoping comes from the field:

```yaml
inheritedBy:
  language: 'rust'
```

Omit it and the task file applies to **every project in the repo**, across all
languages — which silently runs `cargo test` against Python projects, etc.
Always set `inheritedBy`.

### `toolchain: 'system'`

Each task sets `toolchain: 'system'`. This tells moon to run the command with the
ambient PATH (where proto's shims live) rather than a moon-managed toolchain.
proto is the version authority; moon just executes. There is intentionally **no**
`.moon/toolchain.yml`.

### Per-project `moon.yml` is minimal

A project file is just identity + graph metadata:

```yaml
$schema: 'https://moonrepo.dev/schemas/project.json'
layer: 'application'   # or 'library'
language: 'rust'       # drives which task file is inherited
dependsOn:             # OMIT for Rust — inferred from Cargo.toml
  - 'other-project-id'
```

- `layer`: `application` (deployable/leaf) vs `library` (consumed by others).
- `language`: must match an `inheritedBy.language` to receive that language's
  tasks.
- `dependsOn`: **Rust inter-crate deps are inferred from `Cargo.toml`** — leave
  it out for Cargo workspace members. For Python/TS, declare deps by hand so
  `--affected` re-tests dependents.

### Project IDs

In `.moon/workspace.yml`, every project is `id: 'path'`. IDs are global and must
be unique. Convention for nested paths: replace `/` with `-`
(`rose/quartz/frontend` → `quartz-frontend`). A language that lives inside
another language's workspace still gets its own entry (e.g. `cocoon/apps/nyx` is
a Rust crate shipped as a Python wheel — it's `language: 'rust'`).

### Audit tasks are NOT cached

`audit` (cargo-deny / pip-audit) sets `options.cache: false`. The advisory/CVE
database is **external** — not a file input moon can hash — so a cached "pass"
would be stale. Everything else (lint/test/fmt) is cached on inputs.

## lefthook: hooks must be self-sufficient

Git hooks run **non-interactively**: they do NOT source `~/.zshrc` (interactive
-only). So a hook that just calls `ruff` or `moon` will fail on a machine where
those are only on the interactive PATH. Every job therefore prepends proto's
shims explicitly:

```yaml
run: PATH="$HOME/.proto/shims:$PATH" ruff check --fix {staged_files}
```

Division of labour:

- **pre-commit** — fast, file-scoped: format + lint staged files (with
  `stage_fixed: true` so fixes are re-staged), secret scan (gitleaks),
  large-file block. `rustfmt` goes through `moon run :fmt --affected` because
  `cargo fmt` is edition-aware (needs `Cargo.toml`).
- **pre-push** — graph-aware: `moon run :lint --affected` and
  `:test --affected`, across all languages at once. This replaces per-language
  fan-out scripts.

## The developer setup contract

For the stack to work in every shell (including CI and hooks), proto's shims must
be on PATH in `~/.zshenv` (loaded by all shells), not just `~/.zshrc`:

```sh
export PATH="$HOME/.proto/shims:$HOME/.proto/bin:$PATH"
```

The repo README must state this; otherwise hooks/CI "work on my machine" only.

## What to gitignore

```
.moon/cache     # local build/task cache — regenerated
.moon/docker    # docker scaffolding moon generates — regenerated
```

Never commit these. Everything else under `.moon/` and `proto-plugins/` is
committed (it IS the configuration).
