# parle

A native desktop language-learning app, built on [GPUI](https://gpui.rs) —
the GPU-accelerated UI framework behind [Zed](https://zed.dev).

## Why GPUI

We chose to go native rather than wrap a WebView. The deciding factor for a
language app is text: GPUI is what renders Zed's editor, so complex-script
shaping, CJK fallback and input-method editing are already solved. The cost is
documentation — the examples in the crate and Zed's own source are the manual.

## Run

```sh
cargo parle                     # NVIDIA GPU via the workspace alias (needs nvcc at build time)
cargo run -p parle              # CPU inference for the story page
cargo run -p parle --features cuda   # NVIDIA GPU (needs nvcc at build time)
```

Rendering is Vulkan on Linux (via blade), Metal on macOS, DirectX on Windows.
On Linux you need the Vulkan loader, fontconfig, freetype, xkbcommon and
wayland or X11 development libraries — all present on a normal desktop install.

## Pages

**Dictionary** — type a word, pick a language pair, press Enter. Backed by
[MyMemory](https://mymemory.translated.net)'s free API (no key, a few thousand
characters a day). Shows the best translation plus alternatives from its
translation memory.

**Story** — pick a language, describe a story, press Enter. A quantized
[Qwen2.5 0.5B Instruct](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF)
is downloaded once into `~/.cache/parle/` (675 MB) and run locally with
candle, streaming the text into the view as it is generated. It is a baby
model: expect charming nonsense, not literature.

## Layout

```
src/main.rs          entry point, key bindings, window
src/shell.rs         root view: header tabs + active page
src/dictionary.rs    dictionary page + MyMemory client
src/story.rs         story page + worker thread that drives the model
src/llm.rs           GGUF download, candle model, generation loop
src/gguf_tokenizer.rs  rebuild a tokenizer from the GGUF's vocabulary (from augur)
src/text_input.rs    single-line text input (vendored from gpui's example)
src/theme.rs         palette + button/caption helpers
src/lang.rs          the language list
```

## Tests

The network and model tests are opt-in:

```sh
cargo test -p parle -- --ignored                       # translation API
cargo test -p parle --release -- --ignored --nocapture # + download and run the model
```

## GPUI in one paragraph

A view is a struct; `Render::render` builds an element tree from it with a
Tailwind-like builder API (`div().flex().gap_4()`); event handlers mutate the
struct and call `cx.notify()` to re-render. There is no built-in text input —
Zed's editor is its text input — so `text_input.rs` is the crate's example
widget, trimmed and given a `Submit` event. Work that blocks goes on a thread
and comes back through a channel pumped by `cx.spawn`.
