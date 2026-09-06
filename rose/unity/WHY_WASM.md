# Why WebAssembly? Value and use cases in the modern world

This is a companion note to the checkerboard pilot in this folder. The pilot
shows the _mechanics_ of getting Rust into a browser; this note steps back and
asks the harder question: **when is reaching for WASM actually worth it?**

## What WASM actually is

WebAssembly is a compact, portable bytecode with a deterministic execution
model and a sandboxed memory space. The browser was its first host, but the
more durable idea is the format itself: a compilation target that runs at
near-native speed, starts fast, and runs the _same bytes_ anywhere there's a
runtime — browser, server, edge worker, or plugin host.

Two properties do most of the work in every use case below:

1. **Speed** — it's compiled, JIT/AOT-friendly, and avoids the dynamic-typing
   overhead of JavaScript. Predictable performance, not just peak performance.
2. **Sandboxing** — a WASM module can't touch memory, files, or the network
   unless the host explicitly hands it a capability. Untrusted code becomes
   safe-by-default to run.

Everything people get excited about is downstream of one or both of those.

## Where WASM genuinely earns its place

### 1. CPU-heavy work in the browser

This is the pilot's category. When JavaScript is the bottleneck — not the
network, not the DOM, but raw computation — moving the hot path to WASM is a
real win:

- Image/video/audio processing (filters, codecs, transcoding)
- Codecs and compression (e.g. browser builds of ffmpeg)
- 3D, games, physics, CAD (Figma and Photoshop-on-web are the canonical examples)
- Crypto, hashing, simulation, scientific computing
- Parsers and language tooling running client-side

The checkerboard here is a toy version of exactly this: Rust fills a pixel
buffer, JS just blits it to a canvas.

### 2. Reusing an existing non-JS codebase on the web

Often the value isn't speed — it's **not rewriting**. A mature C/C++/Rust
library (a PDF engine, a SQL engine like SQLite via `sql.js`, a regex engine,
a domain solver) can be compiled to WASM and used directly, instead of being
ported to JavaScript and kept in sync forever. One source of truth, two
deployment targets.

### 3. Server-side and edge compute

This is arguably where WASM's _future_ is brightest, not the browser. With
**WASI** (the WebAssembly System Interface), modules run outside the browser
with controlled access to the system. That gives you:

- **Edge functions** (Cloudflare Workers, Fastly Compute) — cold starts in
  microseconds, not the hundreds of milliseconds a container needs, because
  there's no OS image to boot.
- **A lighter alternative to containers** for some workloads: smaller, faster
  to start, more strongly isolated per-tenant.

### 4. Plugin systems and untrusted code

The sandbox is the product here. If you want users (or third parties) to extend
your application with their own logic, WASM lets you run that code without
trusting it. It can't escape its memory or call anything you didn't grant.
Examples: Envoy filters, Shopify Functions, Zellij/Zed plugins, database UDFs,
game modding. Any "let people upload code that runs in our process" feature is a
natural WASM fit.

### 5. Write-once, run-many for SDKs and shared logic

Validation rules, pricing engines, parsers, or business logic that must behave
_identically_ on web, mobile, backend, and CLI can be authored once and compiled
to WASM, eliminating the "the JS and the backend disagree on a rounding rule"
class of bug.

## When _not_ to reach for WASM

WASM is a sharp tool, and most web work doesn't need it. Skip it when:

- **The bottleneck is the DOM or the network.** WASM can't manipulate the DOM
  directly — it has to call back into JS through `wasm-bindgen`-style glue, and
  that boundary has a cost. UI-bound apps gain nothing.
- **The data crossing the JS↔WASM boundary is large or frequent.** Copying
  buffers across the boundary can erase the compute savings. WASM wins when you
  hand it a chunk of work and get a result back, not when you chatter.
- **The work is light.** A few array operations are not worth a toolchain, a
  build step, and a `.wasm` payload to download.
- **Bundle size matters more than CPU.** A WASM module plus its glue is rarely
  smaller than the equivalent JS, and it's an _extra_ download on top of your JS.
- **The team has no systems-language experience.** The productivity hit of
  Rust/C++ tooling can outweigh the runtime gain for non-critical paths.

A good rule of thumb: **profile first.** WASM is an optimisation. If you can't
point at a measured hot path, you're adding complexity for a benefit you haven't
demonstrated.

## The honest trade-offs

- **The boundary is the catch.** Most real-world friction is data marshalling
  across JS↔WASM, not raw execution speed. Design APIs to be coarse-grained.
- **No direct DOM/Web API access.** Everything goes through JS glue.
- **Debugging and observability** are improving but still rougher than native JS
  (source maps, stack traces).
- **Toolchain maturity varies by language.** Rust and C/C++ are first-class;
  others lag or ship large runtimes.
- **The DOM-access and GC proposals are landing gradually**, so some "WASM will
  replace JS" claims are still forward-looking, not today's reality.

## A one-line decision guide

> Reach for WASM when you have **measured, CPU-bound work**, an **existing
> non-JS library worth reusing**, or a need to **run untrusted/portable code
> safely** — and when the work can be handed across the boundary in coarse
> chunks. For everything else, plain JS/TS is simpler and usually enough.

## How this pilot fits

This project is a deliberately minimal instance of category #1: Rust does the
pixel math, JS owns the canvas. It's small enough to show the whole pipeline
(`cargo build` → `wasm-bindgen` → `wasm-opt` → `import` in the browser) without
hiding it behind a framework — which is exactly what you want from a pilot whose
job is to teach the mechanics before you bet a real feature on them.
