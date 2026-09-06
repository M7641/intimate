# vellum

A documentation site rendered **entirely inside the [Bevy](https://bevyengine.org)
game engine** — a website that is also a render loop. There is no HTML and no
DOM: every heading, paragraph, and navigation link is an ECS entity laid out by
Bevy's flexbox UI. The same binary runs natively on the desktop and, compiled to
WebAssembly, inside a browser tab.

This is a `rose/` prototype: an experiment in treating UI as data and giving
ordinary documentation a game engine's capabilities.

## The idea

The whole app follows one rule: **`UI = render(state)`**.

- All content lives in a single `Site` resource (`src/content.rs`), free of any
  Bevy types — the "source document".
- The UI is a pure projection of that resource into `bevy_ui` nodes
  (`src/ui.rs`). Nothing is mutated in place.
- Clicking a nav link only sets `site.current`. A system detects the changed
  resource (`Res::is_changed()`) and rebuilds the entire node tree from scratch.

```
content.rs   pages as plain data (Page → Vec<Block>)
theme.rs     the "stylesheet": colours + font sizes
ui.rs        setup (camera + root) + render_on_change + scroll
field.rs     a WebGPU compute-shader background (see below)
main.rs      App wiring: plugins, resources, systems
```

## WebGPU: a background the GPU computes

Bevy renders through [`wgpu`](https://wgpu.rs), which can target either WebGL2
(the old default) or **WebGPU** in the browser. vellum opts into WebGPU via the
`webgpu` Cargo feature, and `field.rs` puts it to work: a **compute shader**
(`assets/shaders/vellum_field.wgsl`) runs Conway's Game of Life into a storage
texture every frame, displayed as a soft full-bleed glow behind the docs. WebGL2
has no compute stage, so this is a feature that genuinely *needs* the modern API
— not just the same picture drawn a different way.

The simulation lives in Bevy's render world (a render-graph node dispatches the
passes); the main world only holds the two ping-pong textures and a colour
uniform. A translucent scrim over the reading column keeps body text legible
while the field shows through.

## Run it

### Native

```sh
cargo run
```

### In the browser (WebAssembly)

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve --open
```

Trunk compiles the crate to wasm, wires it into `index.html`, copies `assets/`
into `dist/`, and live-reloads on change. Bevy creates the canvas and
`fit_canvas_to_parent` sizes it to the page. The release build
(`trunk build --release`) shrinks the wasm payload via the `opt-level = "s"` +
`lto` profile in `Cargo.toml`.

> **Browser support.** WebGPU needs Chrome/Edge, Safari 18+, or a recent Firefox,
> and a secure context (HTTPS — `localhost` is exempt, so `trunk serve` is fine).
> The backend is chosen at compile time: a WebGPU build does **not** fall back to
> WebGL2. To support older browsers you would ship a second `webgl2` bundle and
> pick between them by feature-detecting `navigator.gpu` in JS.

## Notes

- Text uses Bevy's embedded `default_font`, so no font assets are needed. The
  only asset is the compute shader under `assets/shaders/`; drop a `.ttf` into
  `assets/` and set `TextFont { font, .. }` to customise the type.
- Built against **Bevy 0.18** (0.19 requires rustc ≥ 1.95).
- Scrolling: the content pane uses `Overflow::scroll_y` and a small
  mouse-wheel system updates its `ScrollPosition`.

## Where this could go

- Load pages from Markdown instead of the hard-coded `Site::sample()`.
- Push the compute background further: GPU particles, reaction-diffusion, or a
  flow field reacting to scroll — now that the WebGPU pipeline is in place.
- Animate page transitions with shaders — the whole point of living in a game
  engine.
- Keyboard navigation and an active-link hover state.
