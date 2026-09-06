# AGENTS.md

Guidance for agents working in this repo. Loaded at session start by
`.claude/hooks/load-agents.sh`.

## Prose
Follow the `house-prose` skill for all human-facing text: docs, comments,
commit messages, PRs, chat. No emojis. Minimal words. Short phrases.

## Repo
Polyglot monorepo. Rust-heavy, plus Python, SolidJS, SQL, Typst.
Managed with moon + proto. `.prototools` pins every tool version.
Tool configs live in `.config/` (lefthook, cargo-deny, gitleaks, sqruff), not at
the repo root. Only lefthook auto-discovers it — the other three are invoked with
an explicit `--config`, so keep that flag when you touch a command that runs them.

## Claude config
All of it lives in `.claude/` — `CLAUDE.md`, `skills/`, `hooks/`, `settings.json`.
There is no separate `claude/` directory. The directory is tracked; only
`.claude/settings.local.json` is gitignored.

## After editing
The PostToolUse lint hook formats files automatically (rustfmt, ruff, sqruff,
taplo, prettier — whichever is installed). Do not hand-format.

## Conventions
- Rust does the heavy lifting for code intelligence. No external code-graph tool.
- Add project-specific rules below.
