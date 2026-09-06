# intimate

A polyglot monorepo built around two tools:

- **[proto](https://moonrepo.dev/proto)** — the toolchain manager. One pinned version
  of every tool (rust, python, node, uv, ruff, …) for everyone. Config: `.prototools`.
- **[moon](https://moonrepo.dev/moon)** — the build system. It knows every project and
  how they depend on each other, so it runs/tests/builds **only what your change
  affects** and caches the rest. Config: `.moon/` + a `moon.yml` per project.

Git hooks are managed by **lefthook** (`.config/lefthook.yml`).

This is primarily a playground of ideas, some of which have been worked on over a longer period of time and others which were one shot concepts over the course of an hour. 


## Repo sPrinciples

1. **DX** - does it make developers productive and happy.
2. **UX** - does it provide a great user experience.
3. **Correctness** - does it do the right thing — tests, types, contracts.
4. **Robustness** - does it stay up when inputs and infra misbehave.
5. **Efficient** - does it do what it says it will do and no more.


---

## Setup

```bash
brew install git unzip gzip xz
bash <(curl -fsSL https://moonrepo.dev/install/proto.sh)   # install proto
proto install                                              # install all pinned tools + moon
lefthook install                                           # install the git hooks
```

Make proto available to **all** shells — including non-interactive ones (git hooks,
CI). Add this to `~/.zshenv` (not just `~/.zshrc`, which is interactive-only):

```sh
export PATH="$HOME/.proto/shims:$HOME/.proto/bin:$PATH"
```

Verify:

```bash
moon --version       # should print the version from .prototools
moon query projects  # should list every project in the repo
```

---

## Everyday commands

```bash
moon check --all            # lint + test + typecheck the WHOLE repo ("is it green?")
moon check                  # same, but only what your changes affect

moon run :test --affected   # run one task across only the affected projects
moon run :lint              # run one task across every project
moon run :fmt               # format everything (ruff / cargo fmt)
moon run tako-api:test      # a single project + task

moon query affected         # inspect: what does my current diff impact?
moon query projects         # list all projects
```

`:task` means "this task, across projects". `--affected` filters to the projects your
working changes touch. Re-running an unchanged task is a **cache hit** (instant).

### Tasks available

| Task        | Languages        | Command behind it                                              | In CI? |
| ----------- | ---------------- | ------------------------------------------------------------- | ------ |
| `test`      | python, rust     | `uv run pytest -q` / `cargo nextest run`                      | yes    |
| `test-doc`  | rust             | `cargo test --doc` (doctests — nextest can't run them)        | yes    |
| `lint`      | python, rust, ts | `ruff check` / `cargo clippy --all-targets -D warnings` / `tsc --noEmit` | yes    |
| `typecheck` | python           | `ty check`                                                    | yes    |
| `audit`     | python, rust     | `pip-audit` / `cargo-deny check`                              | yes    |
| `fmt`       | python, rust     | `ruff format` / `cargo fmt`                                   | no¹    |

**Setup tasks run automatically as dependencies** — you don't invoke them directly:
Python `test`/`typecheck` depend on `sync` (`uv sync` for that project), and TypeScript
`lint` depends on each project's `install` (`bun`/`npm install`). So `moon run :test`
materialises the environment for the affected projects first — no manual `uv sync` needed.

> **Project-specific testing tasks.** Beyond the inherited tasks above, individual
> projects add their own per the testing standards: `bench` (perf-gate, in CI),
> `mutation` and `fuzz` (`runInCI: false`, scheduled). These live in a project's own
> `moon.yml`, not the shared language files.

---

## Git hooks (automatic)

Installed by `lefthook install`. They run on every commit/push — no need to invoke moon
by hand for the common path:

- **pre-commit** (on staged files): `ruff check --fix`, `ruff format`, `cargo fmt`,
  `vale` prose lint on Markdown, `gitleaks` secret scan, and a >5 MB file guard.
- **pre-push** (affected only): `moon run :lint --affected` and `:test --affected`.

---

## Prose

**[Vale](https://vale.sh)** lints the Markdown: spelling, plus the house style rules
(no emojis, no hype, no filler). It is pinned in `.prototools` like every other tool.

```bash
vale --config .config/vale.ini README.md          # one file
git ls-files -z '*.md' | xargs -0 vale --config .config/vale.ini   # the whole repo
```

`--config` is mandatory — Vale looks for a `.vale.ini` beside the file, not in
`.config/`. Same story as `gitleaks`, `cargo-deny` and `sqruff`.

| Where | What |
| ----- | ---- |
| `.config/vale.ini` | config: styles, minimum alert level, per-path exceptions |
| `.config/vale/styles/House/` | our rules: `Emoji`, `Hype`, `Filler`, `Passive` |
| `.config/vale/styles/config/vocabularies/House/accept.txt` | project vocabulary |

Only **errors** fail a commit or CI, and spelling is the only error-level rule. Hype
and emoji are warnings; filler and passive voice are suggestions, hidden unless you
ask: `--minAlertLevel=suggestion`.

When Vale flags a word the repo uses on purpose, add it to `accept.txt` — not a
`<!-- vale off -->` comment in the prose. Non-English documents are exempted by path
in `.config/vale.ini`.

---

## Adding a project to the graph

1. Create the project as usual (a `Cargo.toml`, `pyproject.toml`, or `package.json`).
2. Add a `moon.yml` at its root:

   ```yaml
   $schema: "https://moonrepo.dev/schemas/project.json"
   layer: "library" # or 'application'
   language: "rust" # or 'python' / 'typescript' — this is what selects its tasks
   dependsOn: # the other projects it imports (omit if none)
     - "tako-database"
   ```

3. Register it in `.moon/workspace.yml` under `projects:` with a unique id.
4. `moon sync projects` (or just run any moon command) to pick it up.

It now inherits the shared `test`/`lint`/`fmt` tasks for its language, and shows up in
`--affected` whenever it or one of its dependencies changes.
