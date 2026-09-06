# nyx

CLI agents powered by Claude. A collection of specialized terminal commands that use LLMs to automate developer workflows.

## Install

Currently testing the tool as a UV tool which appears to be the best so far as it means it does not interfere with the deployment procceses or depdencies of the project.

```bash
uv add git+https://github.com/nimbus-labs/nimbus-monorepo.git@main#subdirectory=cocoon/nyx

uv tool install  git+https://github.com/nimbus-labs/nimbus-monorepo.git@main#subdirectory=cocoon/nyx
```

### Via pip/uv (recommended)

Compiles the Rust binary and installs it into your Python environment's `bin/`:

```bash
uv pip install cocoon/nyx
```

For development (rebuilds on each invocation):

```bash
cd cocoon/nyx && maturin develop --release
```

### Via cargo

```bash
cargo install --path cocoon/nyx
```

Verify the install and set up shell completions:

```bash
nyx --help
nyx init
```

`nyx init` auto-detects your shell (zsh, bash, fish) and adds tab completions to your rc file. After running it, restart your shell or `source ~/.zshrc` (or equivalent). You only need to do this once — completions auto-update as new commands are added.

## LLM backends

`nyx` auto-detects which backend to use at runtime:

| Priority | Backend                                                         | Requires                                    | How                              |
| -------- | --------------------------------------------------------------- | ------------------------------------------- | -------------------------------- |
| 1st      | Anthropic API (via [Rig](https://github.com/0xPlaygrounds/rig)) | `ANTHROPIC_API_KEY` env var                 | Direct API call, fastest         |
| 2nd      | Claude CLI                                                      | `claude` on PATH (Claude Code subscription) | Runs `claude -p` as a subprocess |

No configuration needed — if `ANTHROPIC_API_KEY` is set, it uses the API; otherwise it falls back to the CLI.

### Setting up each backend

**Claude CLI (subscription, no API key):**

Already works if you have Claude Code installed. Nothing to configure.

**Anthropic API:**

```bash
# Add to your ~/.zshrc or ~/.bashrc
export ANTHROPIC_API_KEY="sk-ant-..."
```

## Setup

### `nyx init`

Sets up tab completion for your shell. Detects zsh/bash/fish automatically and appends the necessary eval line to your rc file.

```bash
nyx init
# → detected shell: zsh
# → added completions to /Users/you/.zshrc
# → restart your shell or run: source ~/.zshrc
```

After setup, `nyx c<TAB>` completes to `clarity`, `commit`, etc. Running `nyx init` again is safe — it skips if completions are already installed.

For manual setup or unsupported shells, use the hidden `completions` command directly:

```bash
nyx completions zsh >> ~/.zshrc        # zsh
nyx completions bash >> ~/.bashrc      # bash
nyx completions fish > ~/.config/fish/completions/nyx.fish  # fish
```

## Commands

### `nyx commit`

Generates a Conventional Commits message from your staged changes and commits.

```bash
# From any git repo:
nyx commit
```

What it does:

1. Stages all changes (`git add -A`)
2. Gets the diff (`git diff --cached`)
3. Sends the diff to Claude with a Conventional Commits prompt
4. Shows you the proposed message
5. You choose:
   - `Y` or Enter — commit with that message
   - `n` — abort
   - `e` — edit the message in `$EDITOR` before committing

Example output:

```
[nyx] using claude CLI (no ANTHROPIC_API_KEY found)
Commit message:

  feat(nyx): add LLM abstraction layer with claude CLI fallback

Commit with this message? [Y/n/e(dit)]
```

## Project structure

```
src/
├── main.rs              # CLI entry point (clap)
├── llm.rs               # LlmClient trait + backends (Claude CLI, Anthropic API, local Candle)
├── model.rs             # Local TinyLlama model loading via Candle
├── ui.rs                # Styled terminal output (banner, prompts, reports)
├── agents/
│   ├── mod.rs
│   ├── commit.rs        # Commit message agent
│   ├── muse.rs          # General Q&A agent
│   └── clarity.rs       # Code clarity scanner agent
└── tools/
    ├── mod.rs
    ├── git.rs            # Git operations (stage, diff, commit)
    ├── picker.rs         # Interactive multi-file selector
    └── shell_init.rs     # Shell completion auto-installer
```

## Architecture

### LLM abstraction

All agents receive a `&dyn LlmClient` — they never depend on a specific provider:

```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn prompt(&self, system: &str, user: &str) -> anyhow::Result<String>;
}
```

Adding a new provider (OpenAI, Ollama, etc.) means implementing this trait — no agent code changes.

### Adding a new agent

1. Create `src/agents/your_agent.rs` with a `pub async fn run(llm: &dyn LlmClient)` function
2. Add `pub mod your_agent;` to `src/agents/mod.rs`
3. Add a variant to the `Commands` enum in `main.rs`
4. Wire it up in the `match`

### Adding a new LLM backend

1. Create a struct implementing `LlmClient` in `src/llm.rs`
2. Update `auto_client()` detection logic (or add a `--backend` CLI flag)

## Dependencies

| Crate                  | Purpose                                |
| ---------------------- | -------------------------------------- |
| `rig-core`             | Anthropic API client + agent framework |
| `tokio`                | Async runtime                          |
| `clap`                 | CLI argument parsing                   |
| `clap_complete`        | Shell completion generation            |
| `async-trait`          | Async methods in traits                |
| `anyhow`               | Error handling                         |
| `candle-core`          | Local model inference (TinyLlama)      |
| `serde` / `serde_json` | JSON parsing (clarity reports)         |

## Requirements

- Rust 1.70+
- One of:
  - Claude Code CLI (`claude`) installed and authenticated
  - `ANTHROPIC_API_KEY` environment variable set

## Idees

1. Scope Review - create tsv.
2. docs into starlight using inertia.
3. Review and update docs
4. Support Checklist Automation
5. Comment Commenter - Identify parts of the code base that don't make sense without further context, add ATTENTION REQUIRED.

### Scope Prompt

```txt
I would like you to create a DELIVERABLES.tsv file that details all the deliverables this application has provided. The outputs should cover:
1. What group the deliverable is in.
2. A concise description of the deliverable.
3. What assumption does the deliverable rely on, for example, what external data or process is required?
Be concise and focus on a director-level or exec-level audience. In this particular case, detail what the optimisation problem solves for.

Use @../daily/DELIVERABLES.tsv as a version to consider when writing the weekly
```
