# augur

A local, low-latency completion server for [Zed](https://zed.dev)'s edit
prediction. It loads a quantized GGUF once into memory (on Metal, on Apple
Silicon) and answers the OpenAI `/v1/completions` requests Zed sends.

## Why this shape

Zed's edit prediction can point at any server implementing OpenAI
`/v1/completions`. So `augur` is deliberately tiny: one endpoint, one model
instance, no database. The latency levers, in the order they matter here:

- **Model loaded once** into an `Arc<Mutex<…>>` — zero per-request load cost.
  The mutex serializes inference, which for a single user is exactly right: it
  stops concurrent requests from fighting over the laptop's memory bandwidth.
- **Quantized weights on Metal** — decode on Apple Silicon is
  memory-bandwidth-bound, not compute-bound, so smaller weights mean
  proportionally more tokens/sec.
- **Small, purpose-built model** — the default is 1.5B, not 4B, which roughly
  cuts prefill cost (the dominant edit-prediction latency) by ~2.6×.

## Run

```sh
cargo run -p augur --release -- serve
# first run downloads the model into ~/.cache/augur/
```

Metal is selected automatically on macOS (compiled in via target-gated deps in
`Cargo.toml`) — no feature flag to remember. On non-macOS it falls back to CPU.

It listens on `http://127.0.0.1:8065` by default. Override with `--host` /
`--port`, and the model with `--model-url` (or `AUGUR_MODEL_URL`). The tokenizer
is rebuilt from the GGUF automatically — no separate file to fetch.

## Point Zed at it

In Zed's `settings.json` (see [Aligning the prompt format](#aligning-zeds-prompt-format-with-the-model) for why `prompt_format` matters):

```json
{
  "features": { "edit_prediction_provider": "open_ai_compatible_api" },
  "edit_predictions": {
    "open_ai_compatible_api": {
      "api_url": "http://127.0.0.1:8065/v1/completions",
      "model": "sweep-next-edit-1.5b",
      "prompt_format": "qwen",
      "max_output_tokens": 256
    }
  }
}
```

## Model — default and alternatives

The default is **Sweep Next-Edit 1.5B**, a Qwen2.5-Coder build purpose-trained
for next-edit prediction (it outperforms models several times its size on
next-edit benchmarks, and is sized to run locally in well under a second).

`augur` reads `general.architecture` from the GGUF header and loads the
matching backend, so **any Qwen2 or Qwen3 GGUF works** — just point
`--model-url` at it. Examples: `Qwen2.5-Coder-3B` (FIM-native), `Qwen3-4B`.

### The tokenizer is rebuilt from the GGUF

Sweep Next-Edit uses a **custom 43,839-token vocabulary** (a distilled vocab,
not Qwen2.5-Coder's 151,643), and its repo ships **only the GGUF** — no
`tokenizer.json`. An off-the-shelf tokenizer would mismatch the model's
embedding table and produce pure garbage.

So `augur` reconstructs the tokenizer from the GGUF's own `tokenizer.ggml.*`
metadata (the byte-level BPE vocab, merges and special tokens) at load time —
the same thing llama.cpp does with the embedded tokenizer, and the reason it
"just works" there. This is automatic for any Qwen-derived GGUF. Pass
`--tokenizer-url` (or `AUGUR_TOKENIZER_URL`) only to override with an external
`tokenizer.json`.

## Aligning Zed's prompt format with the model

Running the model is the easy part. Getting *good* predictions depends on the
prompt Zed sends matching what the model expects. Zed builds the prompt
**client-side** from its `prompt_format` setting and sends a finished string;
`augur` only turns that string into text.

### Recommended: FIM (`prompt_format: "qwen"`)

Sweep is a Qwen2.5-Coder fine-tune and its vocabulary keeps the FIM tokens
(`<|fim_prefix|>`, `<|fim_suffix|>`, `<|fim_middle|>`). Zed's `qwen` format emits
exactly those, and its output contract (return the text between prefix and
suffix) matches what `augur` returns. This works today:

```json
{
  "features": { "edit_prediction_provider": "open_ai_compatible_api" },
  "edit_predictions": {
    "open_ai_compatible_api": {
      "api_url": "http://127.0.0.1:8065/v1/completions",
      "model": "sweep-next-edit-1.5b",
      "prompt_format": "qwen",
      "max_output_tokens": 256
    }
  }
}
```

Verified: `<|fim_prefix|>def is_even(n):\n    return <|fim_suffix|>…<|fim_middle|>`
→ `n % 2 == 0`. Quality is good-but-not-perfect: Sweep was trained mainly on its
own next-edit format, so pure FIM is functional, not its best mode.

### Sweep's native format (for its best quality)

Sweep's strongest mode is its own `<|file_sep|>` next-edit format, which is fed
the *recent edit history* (not just a cursor split):

```
<|file_sep|>{context_file}            ← related files
{content}
<|file_sep|>{file}.diff               ← a recent change, as a pattern
original:
{before}
updated:
{after}
<|file_sep|>original/{path}           ← the edited file, three states
{before_recent_change}
<|file_sep|>current/{path}
{current}
<|file_sep|>updated/{path}            ← model completes here, stops at <|file_sep|>
```

Zed's `zeta2` format also carries recent-edit history, so it can in principle be
translated into this. To build that translation correctly we need to see Zed's
*actual* `zeta2` output, which varies by version — so capture it:

```sh
augur serve --log-prompts          # writes ~/.cache/augur/prompts.jsonl
```

Set `prompt_format: "zeta2"` in Zed, trigger a few predictions, then inspect
`prompts.jsonl`. Those real samples are what a `zeta2 → <|file_sep|>` reformatter
in `augur` would be built and tested against.

This alignment question is independent of the runtime — anyone running Sweep
under llama.cpp hits it too.

## Latency (measured, M4, warm)

| Context | Sweep Next-Edit 1.5B (Q8) | Qwen3-4B (Q4) |
|---|---|---|
| Small (~20 tok) | ~130 ms | ~150 ms |
| Next-edit (~250 tok) | ~280–700 ms | — |
| Large (~1400 tok) | ~1.7 s | ~3.3 s |

(Cold start adds ~5 s once, compiling Metal pipelines.) The large-context
number is **prefill-bound and recomputed every request** — the headline cost
for edit prediction, which resends a big context on every keystroke. The 1.5B
default roughly halves it versus the 4B; a prefix cache (below) would mostly
erase it.

## Known limits / next latency wins

- **No prefix/KV cache reuse across requests.** This would be the single biggest
  win — caching the unchanged prefix so prefill is near-zero on each keystroke.
  It is **not implementable on candle's quantized models** through their public
  API: the KV cache supports only full append and full reset, not truncation to
  an arbitrary prefix length, which prefix reuse requires. Doing it properly
  means either vendoring/forking the model code to expose cache slicing, or
  moving the runtime to `llama.cpp` (via `llama-cpp-2`), whose slot-based prompt
  cache handles exactly this. That is the upgrade path if 1.5B prefill on large
  contexts is still too slow.
- **No speculative decoding.** A tiny draft model drafting for the target would
  speed up decode (Sweep's own <500 ms figure assumes it).
- **No streaming.** Edit-prediction completions are short (≤256 tokens), so the
  full response is returned at once.
