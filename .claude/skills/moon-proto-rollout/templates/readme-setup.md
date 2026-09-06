<!-- Paste into the target repo's README, adjusting the intro to the repo. -->

A polyglot monorepo built around two tools:

- **[proto](https://moonrepo.dev/proto)** — the toolchain manager. One pinned
  version of every tool (rust, python, node, uv, ruff, …) for everyone.
  Config: `.prototools`.
- **[moon](https://moonrepo.dev/moon)** — the build system. It knows every
  project and how they depend on each other, so it runs/tests/builds **only what
  your change affects** and caches the rest. Config: `.moon/` + a `moon.yml` per
  project.

Git hooks are managed by **lefthook** (`lefthook.yml`).

## Setup

```bash
brew install git unzip gzip xz                              # archive tools proto needs
bash <(curl -fsSL https://moonrepo.dev/install/proto.sh)    # install proto
proto install                                              # install all pinned tools + moon
lefthook install                                           # install the git hooks
```

Make proto available to **all** shells — including non-interactive ones (git
hooks, CI). Add this to `~/.zshenv` (not just `~/.zshrc`, which is
interactive-only):

```sh
export PATH="$HOME/.proto/shims:$HOME/.proto/bin:$PATH"
```

Verify:

```bash
moon --version       # should print the version from .prototools
moon query projects  # should list every project in the repo
```

## Everyday commands

```bash
moon run :lint --affected     # lint only what changed (all languages)
moon run :test --affected     # test only what changed
moon run :fmt --affected      # format
moon run <project>:<task>     # run one task for one project
```
